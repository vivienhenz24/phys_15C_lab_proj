use hound::WavReader;
use realfft::RealFftPlanner;
use std::path::{Path, PathBuf};
use std::ffi::OsStr;
use std::fs::File;
use std::io::Write;

const SAMPLE_DIVISOR: f32 = 32768.0;
const START_BIN: usize = 10;
const PILOT_PATTERN: [u8; 8] = [0, 1, 0, 1, 0, 1, 0, 1];
const LENGTH_HEADER_BITS: usize = 16;
const BAND_OFFSET: usize = 4; // skip a few noisy bins near START_BIN

pub struct DecodedWatermark {
    pub message: String,
    pub raw_bytes: Vec<u8>,
}

#[derive(Clone, Copy)]
pub struct DecodeConfig {
    pub frame_duration: f32, // seconds
    pub strength: f32,       // 0.0 - 1.0 (hint only)
    pub sample_rate_hint: Option<u32>,
}

impl DecodeConfig {
    pub fn new(frame_duration_ms: u32, strength_percent: u32) -> Self {
        Self {
            frame_duration: (frame_duration_ms.max(1) as f32) / 1000.0,
            strength: (strength_percent as f32 / 100.0).clamp(0.01, 0.5),
            sample_rate_hint: None,
        }
    }
    pub fn with_sample_rate(mut self, sample_rate: u32) -> Self {
        self.sample_rate_hint = Some(sample_rate);
        self
    }
}

impl Default for DecodeConfig {
    fn default() -> Self {
        Self {
            frame_duration: 0.032,
            strength: 0.15,
            sample_rate_hint: None,
        }
    }
}

pub fn default_watermarked_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("output_data")
        .join("spectrum")
        .join("OSR_us_000_0057_8k_watermarked.wav")
}

pub fn decode_spread_spectrum(path: impl AsRef<Path>, config: DecodeConfig) -> DecodedWatermark {
    println!("=== Spread-Spectrum Watermark Decoder ===\n");
    let in_path = path.as_ref();
    let (samples, sample_rate) = load_audio(in_path);
    let debug = std::env::var("SS_DEBUG").is_ok();
    let dump = std::env::var("SS_DUMP").is_ok();
    if let Some(expected_rate) = config.sample_rate_hint {
        if sample_rate != expected_rate {
            println!(
                "Note: filename implies {} Hz but file reports {} Hz",
                expected_rate, sample_rate
            );
        }
    }

    let frame_len = ((sample_rate as f32 * config.frame_duration).round().max(1.0)) as usize;
    if frame_len <= START_BIN {
        panic!(
            "frame length {} (from {:.3}s @ {} Hz) insufficient for decoding",
            frame_len, config.frame_duration, sample_rate
        );
    }

    let mut planner = RealFftPlanner::<f32>::new();
    let forward = planner.plan_fft_forward(frame_len);
    let mut buffer = vec![0.0f32; frame_len];
    let mut spectrum = forward.make_output_vec();
    let mut scratch = forward.make_scratch_vec();

    const BAND_OFFSET: usize = 4;
    let usable = spectrum.len().saturating_sub(START_BIN + BAND_OFFSET);
    let desired_band = 64usize;
    let band_len = desired_band.min(usable.max(1));
    let window_radius = ((usable / 20).max(1)).min(10);
    if debug {
        println!(
            "FFT len: {}, usable bins: {}, band_len: {}, window_radius: {}",
            frame_len, usable, band_len, window_radius
        );
    }

    let mut debug_rows: Vec<(usize, &'static str, f32, f32, f32)> = Vec::new();

    // repetition factor (frames per bit)
    let repeat = std::env::var("SS_REPEAT").ok().and_then(|s| s.parse::<usize>().ok()).unwrap_or(3).max(1);
    let pilot_frames = PILOT_PATTERN.len() * repeat;
    let length_frames = LENGTH_HEADER_BITS * repeat;

    // 1) Collect all frames we'll need: pilot + length + some data
    let mut offset = 0usize;
    let mut frame_index = 0usize;
    let total_budget = pilot_frames + length_frames + 64 * repeat + (repeat - 1);
    let mut all_frames_raw: Vec<(Vec<f32>, usize)> = Vec::with_capacity(total_budget); // (scores, global_frame_idx)
    
    while offset < samples.len() && all_frames_raw.len() < total_budget {
        let end = (offset + frame_len).min(samples.len());
        let frame = &samples[offset..end];
        buffer.fill(0.0);
        buffer[..frame.len()].copy_from_slice(frame);
        forward
            .process_with_scratch(&mut buffer, &mut spectrum, &mut scratch)
            .expect("FFT failed");
        let scores = band_scores(&spectrum, usable, window_radius);
        all_frames_raw.push((scores, frame_index));
        offset += frame_len;
        frame_index += 1;
    }
    
    if all_frames_raw.len() < pilot_frames {
        panic!("insufficient frames for pilot");
    }
    
    // Pilot alignment search: for each shift s, compute correlations using encoder's seeding
    let mut best_shift = 0usize;
    let mut best_matches = 0i32;
    let mut best_mag = 0.0f32;
    if debug { println!("Pilot alignment scan (repeat={}):  ", repeat); }
    for s in 0..repeat.min(all_frames_raw.len()) {
        let mut matches = 0i32;
        let mut mag_acc = 0.0f32;
        let mut decided_bits = Vec::new();
        for b in 0..PILOT_PATTERN.len() {
            let mut group_corrs: Vec<f32> = Vec::new();
            for r in 0..repeat {
                let fidx = s + b * repeat + r;
                if fidx >= all_frames_raw.len() { break; }
                let (ref sc, _) = all_frames_raw[fidx];
                let bit_seed = b as u32; // encoder uses bit index as seed
                let pos = corr_scores(sc, band_len, 1.0, bit_seed);
                let neg = corr_scores(sc, band_len, -1.0, bit_seed);
                let signed = pos - neg;
                group_corrs.push(signed);
                if debug && s == 0 && b == 0 && r == 0 {
                    println!("  [Trace] shift=0, bit=0, rep=0, fidx={}, bit_seed={}, pos={:.3}, neg={:.3}, signed={:.3}", fidx, bit_seed, pos, neg, signed);
                }
            }
            if group_corrs.is_empty() { continue; }
            let mean = group_corrs.iter().sum::<f32>() / group_corrs.len() as f32;
            let decided = u8::from(mean >= 0.0);
            decided_bits.push(decided);
            if decided == PILOT_PATTERN[b] { matches += 1; }
            mag_acc += mean.abs();
        }
        if debug {
            println!("  shift={}: matches={}/8, bits={:?}", s, matches, decided_bits);
        }
        if matches > best_matches || (matches == best_matches && mag_acc > best_mag) {
            best_matches = matches;
            best_mag = mag_acc;
            best_shift = s;
        }
    }
    
    if debug {
        println!("Pilot alignment: best_shift={}, matches={}/8, mag={:.3}", best_shift, best_matches, best_mag);
    }
    
    // Build aligned pilot signed correlations
    let mut pilot_signed: Vec<f32> = Vec::new();
    for b in 0..PILOT_PATTERN.len() {
        for r in 0..repeat {
            let fidx = best_shift + b * repeat + r;
            if fidx >= all_frames_raw.len() { break; }
            let (ref sc, gidx) = all_frames_raw[fidx];
            let bit_seed = b as u32;
            let pos = corr_scores(sc, band_len, 1.0, bit_seed);
            let neg = corr_scores(sc, band_len, -1.0, bit_seed);
            let signed = pos - neg;
            pilot_signed.push(signed);
            if dump { debug_rows.push((gidx, "pilot", pos, neg, signed)); }
        }
    }

    // Majority vote per bit across repetitions
    let mut pilot_bits: Vec<u8> = Vec::with_capacity(PILOT_PATTERN.len());
    for b in 0..PILOT_PATTERN.len() {
        let start = b * repeat;
        let end = start + repeat;
        let mean = pilot_signed[start..end].iter().copied().sum::<f32>() / repeat as f32;
        pilot_bits.push(u8::from(mean >= 0.0));
    }
    if debug {
        println!("Pilot decided bits: {:?}", pilot_bits);
        println!("Pilot expected bits: {:?}", PILOT_PATTERN);
    }
    // stats for logging
    let mut ones = Vec::new();
    let mut zeros = Vec::new();
    for (b, &want) in PILOT_PATTERN.iter().enumerate() {
        let start = b * repeat;
        let end = start + repeat;
        let mean = pilot_signed[start..end].iter().copied().sum::<f32>() / repeat as f32;
        if want == 1 { ones.push(mean); } else { zeros.push(mean); }
    }
    let avg_one = if ones.is_empty() { 0.0 } else { ones.iter().sum::<f32>() / ones.len() as f32 };
    let avg_zero = if zeros.is_empty() { 0.0 } else { zeros.iter().sum::<f32>() / zeros.len() as f32 };
    // With differential correlation, 0 is the natural boundary; keep logs for debug
    println!(
        "Pilot signed corr -> one: {:.6}, zero: {:.6} (bit decided by sign)",
        avg_one, avg_zero
    );
    // Polarity: if zeros are more positive than ones, flip the sign for subsequent decisions
    let polarity: f32 = if avg_one >= avg_zero { 1.0 } else { -1.0 };
    if debug { println!("Polarity: {}", polarity); }

    // 2) Extract and decode length header using same shift
    let length_start = best_shift + pilot_frames;
    let mut length_signed: Vec<f32> = Vec::new();
    for b in 0..LENGTH_HEADER_BITS {
        for r in 0..repeat {
            let fidx = length_start + b * repeat + r;
            if fidx >= all_frames_raw.len() { break; }
            let (ref sc, gidx) = all_frames_raw[fidx];
            let bit_seed = (PILOT_PATTERN.len() + b) as u32; // encoder seed: 8..23
            let pos = corr_scores(sc, band_len, 1.0, bit_seed);
            let neg = corr_scores(sc, band_len, -1.0, bit_seed);
            let signed = polarity * (pos - neg);
            length_signed.push(signed);
            if debug && b == 0 && r == 0 {
                println!("  [Trace Length] bit={}, rep={}, fidx={}, gidx={}, bit_seed={}, pos={:.3}, neg={:.3}, signed={:.3}", b, r, fidx, gidx, bit_seed, pos, neg, signed);
            }
            if dump { debug_rows.push((gidx, "len", pos, neg, signed)); }
        }
    }
    
    let mut length_bits: Vec<u8> = Vec::new();
    for b in 0..LENGTH_HEADER_BITS {
        let start = b * repeat;
        let end = start + repeat;
        if end > length_signed.len() { length_bits.push(0); continue; }
        let mean = length_signed[start..end].iter().sum::<f32>() / repeat as f32;
        length_bits.push(u8::from(mean >= 0.0));
    }
    if debug {
        let len_means: Vec<f32> = (0..LENGTH_HEADER_BITS).map(|b| {
            let start = b * repeat;
            let end = (start + repeat).min(length_signed.len());
            if end > start { length_signed[start..end].iter().sum::<f32>() / (end - start) as f32 } else { 0.0 }
        }).collect();
        println!("Length group means: {:?}", len_means);
        println!("Length bits: {:?}", length_bits);
        println!("Expected length bits for 8 bytes: [0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0]");
        let mut check_val = 0u16;
        for &b in length_bits.iter() { check_val = (check_val << 1) | (b as u16); }
        println!("Length bits decode to: {} (hex: 0x{:04x})", check_val, check_val);
    }

    if length_bits.len() < LENGTH_HEADER_BITS {
        println!("Warning: incomplete length header; assuming zero length");
    }
    let message_len_raw = decode_length_header(&length_bits);
    let max_len = std::env::var("SS_MAX_LEN")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(256);
    let message_len = message_len_raw.min(max_len);
    println!("Length header reports {} message bytes", message_len);
    if debug {
        println!("Length raw value: {} -> clamped to {}", message_len_raw, message_len);
    }

    // 3) Extract and decode payload using same shift
    let data_start = best_shift + pilot_frames + length_frames;
    let data_bits_needed = message_len.saturating_mul(8);
    let mut data_signed: Vec<f32> = Vec::new();
    for b in 0..data_bits_needed {
        for r in 0..repeat {
            let fidx = data_start + b * repeat + r;
            if fidx >= all_frames_raw.len() { break; }
            let (ref sc, gidx) = all_frames_raw[fidx];
            let bit_seed = (PILOT_PATTERN.len() + LENGTH_HEADER_BITS + b) as u32; // encoder seed: 24+
            let pos = corr_scores(sc, band_len, 1.0, bit_seed);
            let neg = corr_scores(sc, band_len, -1.0, bit_seed);
            let signed = polarity * (pos - neg);
            data_signed.push(signed);
            if dump { debug_rows.push((gidx, "data", pos, neg, signed)); }
        }
    }
    
    let mut data_bits: Vec<u8> = Vec::new();
    for b in 0..data_bits_needed {
        let start = b * repeat;
        let end = start + repeat;
        if end > data_signed.len() { break; }
        let mean = data_signed[start..end].iter().sum::<f32>() / repeat as f32;
        data_bits.push(u8::from(mean >= 0.0));
    }
    let data_signed_preview: Vec<f32> = data_signed.iter().copied().take(32).collect();

    println!("Recovered {} data bits", data_bits.len());
    if debug {
        println!("Data signed preview (first 32): {:?}", data_signed_preview);
        println!("Data bits preview (first 64): {:?}", &data_bits[..data_bits.len().min(64)]);
    }
    let decoded = bits_to_message(data_bits);
    println!("\nDecoded message: \"{}\" (bytes: {:?})", decoded.message, decoded.raw_bytes);
    println!("\n=== Decoding Complete ===");

    if dump {
        if let Some(stem) = in_path.file_stem().and_then(OsStr::to_str) {
            let out_csv = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("output_data")
                .join("spectrum")
                .join(format!("ss_debug_{}.csv", stem));
            if let Err(e) = write_debug_csv(&out_csv, &debug_rows) {
                eprintln!("Failed to write debug CSV {}: {}", out_csv.display(), e);
            } else if debug {
                println!("Wrote debug CSV to {}", out_csv.display());
            }
        }
    }
    decoded
}

fn pn_correlation(spectrum: &[realfft::num_complex::Complex32], band_len: usize, sign: f32, seed: u32) -> f32 {
    // Use log-magnitude for dynamic range stability; correlate with PN chips
    let epsilon = 1e-12f32;
    let mut acc = 0.0f32;
    for k in 0..band_len {
        let idx = START_BIN + k;
        if idx >= spectrum.len() { break; }
        let mag = spectrum[idx].norm().max(epsilon).ln();
        let pn = pn_value(seed, k as u32);
        acc += sign * pn * mag;
    }
    acc / (band_len as f32).sqrt()
}

fn band_scores(spectrum: &[realfft::num_complex::Complex32], usable: usize, window_radius: usize) -> Vec<f32> {
    // Local-mean normalized log-magnitudes
    let epsilon = 1e-12f32;
    let mut logs = Vec::with_capacity(usable);
    for k in 0..usable {
        let idx = START_BIN + BAND_OFFSET + k;
        if idx >= spectrum.len() { break; }
        logs.push(spectrum[idx].norm().max(epsilon).ln());
    }
    let n = logs.len();
    let mut prefix = vec![0f64; n + 1];
    for (i, &v) in logs.iter().enumerate() { prefix[i + 1] = prefix[i] + v as f64; }
    let mut scores = Vec::with_capacity(n);
    for i in 0..n {
        let start = i.saturating_sub(window_radius);
        let end = (i + window_radius + 1).min(n);
        let neighbours = end.saturating_sub(start + 1);
        if neighbours == 0 { scores.push(0.0); continue; }
        let sum = prefix[end] - prefix[start] - logs[i] as f64;
        let mean = sum / neighbours as f64;
        scores.push(logs[i] - mean as f32);
    }
    scores
}

fn corr_scores(scores: &[f32], band_len: usize, sign: f32, seed: u32) -> f32 {
    let mut acc = 0.0f32;
    let len = band_len.min(scores.len());
    for k in 0..len {
        let pn = pn_value(seed, k as u32);
        acc += sign * pn * scores[k];
    }
    acc / (len as f32).sqrt()
}

fn write_debug_csv(path: &Path, rows: &[(usize, &'static str, f32, f32, f32)]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
    let mut f = File::create(path)?;
    writeln!(f, "frame_index,stage,pos,neg,signed")?;
    for (idx, stage, pos, neg, signed) in rows.iter() {
        writeln!(f, "{},{},{:.6},{:.6},{:.6}", idx, stage, pos, neg, signed)?;
    }
    Ok(())
}

#[inline]
fn pn_value(bit_seed: u32, tap: u32) -> f32 {
    let mut x = bit_seed
        .wrapping_mul(0x9E3779B9)
        .wrapping_add(tap.wrapping_mul(0x85EBCA6B));
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    if (x & 1) == 0 { 1.0 } else { -1.0 }
}

fn decode_length_header(bits: &[u8]) -> usize {
    let mut len = 0u16;
    for &b in bits.iter().take(LENGTH_HEADER_BITS) {
        len = (len << 1) | u16::from(b & 1);
    }
    len as usize
}

fn bits_to_message(bits: Vec<u8>) -> DecodedWatermark {
    let mut bytes = Vec::with_capacity((bits.len() + 7) / 8);
    for chunk in bits.chunks(8) {
        let mut byte = 0u8;
        for &b in chunk {
            byte = (byte << 1) | (b & 1);
        }
        bytes.push(byte);
    }
    DecodedWatermark {
        message: String::from_utf8_lossy(&bytes).into_owned(),
        raw_bytes: bytes,
    }
}

fn load_audio(path: &Path) -> (Vec<f32>, u32) {
    println!("Loading watermarked audio from {}", path.display());
    let mut reader = WavReader::open(path).expect("failed to open watermarked wav");
    let spec = reader.spec();
    let samples: Vec<f32> = reader
        .samples::<i16>()
        .map(|s| s.expect("failed to read sample") as f32 / SAMPLE_DIVISOR)
        .collect();
    println!("Loaded {} samples at {} Hz", samples.len(), spec.sample_rate);
    (samples, spec.sample_rate)
}


