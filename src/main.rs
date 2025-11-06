// Import the standard library's environment module for reading command-line arguments
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use hound::{SampleFormat, WavReader, WavSpec, WavWriter};

// Import modules we defined in separate files
mod decoder; // Contains all decoding logic
mod encoder; // Contains all encoding logic
mod spread_spectrum_decoder; // Spread-spectrum decoder
mod spread_spectrum_encoder; // Spread-spectrum encoder

// =============================================================================
// Entry point - runs encode or decode based on command
// =============================================================================

fn main() {
    let mut args = env::args();
    let _program = args.next(); // program name

    let Some(command) = args.next() else {
        print_usage();
        return;
    };

    match command.as_str() {
        "hello" => encoder::encode_sample("hello"),
        "encode" => encoder::encode_sample("fourrier"),
        "encode-ss" => spread_spectrum_encoder::encode_spread_spectrum("fourrier"),
        "decode" => {
            let (path, config) = decode_target(args.next());
            decoder::decode_watermarked_sample(path, config);
        }
        "decode-ss" => {
            let (path, config) = decode_target_ss(args.next());
            spread_spectrum_decoder::decode_spread_spectrum(path, config);
        }
        "decode-ss_unknown" => {
            decode_spread_spectrum_unknown(args.next());
        }
        _ => print_usage(),
    }
}

fn print_usage() {
    println!("usage:");
    println!("  cargo run -- encode");
    println!("  cargo run -- encode-ss");
    println!("  cargo run -- decode [filename]");
    println!("  cargo run -- decode-ss [filename]");
    println!("  cargo run -- decode-ss_unknown [filename]");
    println!();
    println!("If no filename is provided, the decoder uses the default output.");
    println!("When a filename is provided, it is looked up in the output_data folder.");
}

fn decode_target(arg: Option<String>) -> (PathBuf, decoder::DecodeConfig) {
    match arg {
        Some(file) => {
            let config = parse_config(&file);
            let path = if Path::new(&file).is_absolute() {
                PathBuf::from(&file)
            } else {
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("output_data")
                    .join(&file)
            };
            (path, config)
        }
        None => (
            decoder::default_watermarked_path(),
            decoder::default_config(),
        ),
    }
}

fn parse_config(filename: &str) -> decoder::DecodeConfig {
    let stem = Path::new(filename)
        .file_stem()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();
    let mut segments = stem.split('_');
    let sample_rate = segments
        .next()
        .and_then(|part| part.parse::<u32>().ok())
        .unwrap_or(8_000);
    let frame_ms = segments
        .next()
        .and_then(|part| part.parse::<u32>().ok())
        .unwrap_or(32);
    let strength_percent = segments
        .next()
        .and_then(|part| part.parse::<u32>().ok())
        .unwrap_or(15);

    decoder::DecodeConfig::new(frame_ms, strength_percent).with_sample_rate(sample_rate)
}

fn decode_target_ss(arg: Option<String>) -> (PathBuf, spread_spectrum_decoder::DecodeConfig) {
    match arg {
        Some(file) => {
            let config = parse_config_ss(&file);
            let provided = Path::new(&file);
            let mut candidates: Vec<PathBuf> = Vec::new();
            if provided.is_absolute() {
                candidates.push(provided.to_path_buf());
            } else {
                let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
                candidates.push(manifest.join("output_data").join("spectrum").join(&file));
                candidates.push(manifest.join(&file));
                candidates.push(PathBuf::from(&file));
            }
            let path = candidates
                .into_iter()
                .find(|candidate| candidate.exists())
                .unwrap_or_else(|| {
                    if provided.is_absolute() {
                        provided.to_path_buf()
                    } else {
                        Path::new(env!("CARGO_MANIFEST_DIR"))
                            .join("output_data")
                            .join("spectrum")
                            .join(&file)
                    }
                });
            (path, config)
        }
        None => (
            spread_spectrum_decoder::default_watermarked_path(),
            spread_spectrum_decoder::DecodeConfig::default(),
        ),
    }
}

fn parse_config_ss(filename: &str) -> spread_spectrum_decoder::DecodeConfig {
    let stem = Path::new(filename)
        .file_stem()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();
    let mut segments = stem.split('_');
    let sample_rate = segments
        .next()
        .and_then(|part| part.parse::<u32>().ok())
        .unwrap_or(8_000);
    let frame_ms = segments
        .next()
        .and_then(|part| part.parse::<u32>().ok())
        .unwrap_or(32);
    let strength_percent = segments
        .next()
        .and_then(|part| part.parse::<u32>().ok())
        .unwrap_or(15);

    spread_spectrum_decoder::DecodeConfig::new(frame_ms, strength_percent)
        .with_sample_rate(sample_rate)
}

fn decode_spread_spectrum_unknown(arg: Option<String>) {
    let (path, mut seed_config) = decode_target_ss(arg);
    if !path.exists() {
        eprintln!("File not found: {}", path.display());
        return;
    }

    let mut working_path = path.clone();
    let mut derived_sample_rate = None;
    let mut temp_guard = TempPathGuard::default();

    if is_csv_path(&working_path) {
        match csv_to_temp_wav(&working_path) {
            Ok(conv) => {
                let CsvConversion {
                    path: temp_path,
                    sample_rate: sr,
                    samples,
                } = conv;
                println!(
                    "Converted CSV {} to temporary WAV {} ({} samples @ {} Hz)",
                    working_path.display(),
                    temp_path.display(),
                    samples,
                    sr
                );
                derived_sample_rate = Some(sr);
                temp_guard.set(temp_path.clone());
                working_path = temp_path;
            }
            Err(msg) => {
                eprintln!("{msg}");
                return;
            }
        }
    }

    if let Some(sr) = derived_sample_rate {
        seed_config.sample_rate_hint = Some(sr);
    }

    let mut actual_sample_rate = read_wav_sample_rate(&working_path);
    if derived_sample_rate.is_none() {
        derived_sample_rate = actual_sample_rate;
    }
    if seed_config.sample_rate_hint.is_none() {
        if let Some(sr) = actual_sample_rate {
            seed_config.sample_rate_hint = Some(sr);
        }
    }
    if actual_sample_rate.is_none() {
        actual_sample_rate = seed_config.sample_rate_hint;
    }

    let base_frame_ms = (seed_config.frame_duration * 1000.0).round().max(1.0) as u32;
    let base_strength = (seed_config.strength * 100.0).round() as u32;

    let frame_candidates_raw = [
        base_frame_ms,
        10,
        12,
        16,
        20,
        24,
        28,
        32,
        36,
        40,
        44,
        48,
        56,
        64,
        72,
        80,
        96,
        112,
        128,
    ];
    let strength_candidates_raw = [
        base_strength,
        5,
        8,
        10,
        12,
        15,
        18,
        20,
        25,
        30,
        35,
        40,
        45,
        50,
    ];

    let mut frame_candidates: Vec<u32> = frame_candidates_raw
        .into_iter()
        .filter(|&ms| ms >= 1 && ms <= 256)
        .collect();
    frame_candidates.sort_unstable();
    frame_candidates.dedup();

    let mut strength_candidates: Vec<u32> = strength_candidates_raw
        .into_iter()
        .map(|s| s.clamp(1, 100))
        .collect();
    strength_candidates.sort_unstable();
    strength_candidates.dedup();

    let mut sample_rate_candidates: Vec<Option<u32>> = Vec::new();
    let mut sr_seen = std::collections::HashSet::new();
    if let Some(sr) = actual_sample_rate {
        if sr_seen.insert(Some(sr)) {
            sample_rate_candidates.push(Some(sr));
        }
    }
    if let Some(sr) = derived_sample_rate {
        if sr_seen.insert(Some(sr)) {
            sample_rate_candidates.push(Some(sr));
        }
    }
    if let Some(rate) = seed_config.sample_rate_hint {
        if sr_seen.insert(Some(rate)) {
            sample_rate_candidates.push(Some(rate));
        }
    }
    if sr_seen.insert(None) {
        sample_rate_candidates.push(None);
    }

    let original_repeat_env = std::env::var("SS_REPEAT").ok();
    let seed_repeat = original_repeat_env
        .as_deref()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(3);
    let repeat_candidates_raw = [seed_repeat, 1, 2, 3, 4, 5, 6, 8];
    let mut repeat_candidates: Vec<u32> = repeat_candidates_raw.into_iter().collect();
    repeat_candidates.sort_unstable();
    repeat_candidates.dedup();

    struct RepeatGuard(Option<String>);
    impl Drop for RepeatGuard {
        fn drop(&mut self) {
            match &self.0 {
                Some(val) => std::env::set_var("SS_REPEAT", val),
                None => std::env::remove_var("SS_REPEAT"),
            }
        }
    }
    let _guard = RepeatGuard(original_repeat_env);

    let mut attempted = std::collections::HashSet::new();
    let max_len_limit = std::env::var("SS_MAX_LEN")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(256);
    let mut best_result: Option<(
        usize,
        usize,
        spread_spectrum_decoder::DecodedWatermark,
        u32,
        u32,
        u32,
        Option<u32>,
    )> = None;
    let total_attempts = frame_candidates.len()
        * strength_candidates.len()
        * sample_rate_candidates.len()
        * repeat_candidates.len();
    println!(
        "Attempting spread-spectrum decode with up to {} combinations...",
        total_attempts
    );

    for &repeat in &repeat_candidates {
        std::env::set_var("SS_REPEAT", repeat.to_string());
        for &frame_ms in &frame_candidates {
            for &strength in &strength_candidates {
                for &sample_rate in &sample_rate_candidates {
                    let key = (repeat, frame_ms, strength, sample_rate);
                    if !attempted.insert(key) {
                        continue;
                    }
                    let sr_label_owned = sample_rate
                        .map(|sr| sr.to_string())
                        .unwrap_or_else(|| "auto".to_string());
                    let sr_label = sr_label_owned.as_str();
                    println!(
                        "\n=== Trying: repeat={} frames, frame={} ms, strength={}%, sample_rate_hint={} ===",
                        repeat,
                        frame_ms,
                        strength,
                        sr_label
                    );
                    let sr_for_len = sample_rate.or(actual_sample_rate).unwrap_or(8_000);
                    let frame_len_est = (((sr_for_len as f32) * (frame_ms as f32) / 1000.0)
                        .round()
                        .max(1.0)) as usize;
                    if frame_len_est <= 10 {
                        println!(
                            "Skipping combination repeat={}, frame={} ms, strength={}%, sample_rate_hint={} (estimated frame length {} ≤ 10 bins)",
                            repeat,
                            frame_ms,
                            strength,
                            sr_label,
                            frame_len_est
                        );
                        continue;
                    }

                    let mut candidate =
                        spread_spectrum_decoder::DecodeConfig::new(frame_ms, strength);
                    if let Some(rate) = sample_rate {
                        candidate = candidate.with_sample_rate(rate);
                    }
                    let attempt = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        spread_spectrum_decoder::decode_spread_spectrum(&working_path, candidate)
                    }));
                    match attempt {
                        Ok(decoded) => {
                            let len = decoded.raw_bytes.len();
                            let replacement_count =
                                decoded.message.chars().filter(|&c| c == '\u{FFFD}').count();
                            let looks_plausible =
                                len > 0 && len < max_len_limit && replacement_count == 0;
                            if looks_plausible {
                                println!(
                                    "\nSuccessful decode with repeat={}, frame={} ms, strength={}%, sample_rate_hint={}:\n\"{}\" ({} bytes)",
                                    repeat,
                                    frame_ms,
                                    strength,
                                    sr_label,
                                    decoded.message,
                                    len
                                );
                                return;
                            }
                            let score = (replacement_count, len);
                            let should_replace = match &best_result {
                                None => true,
                                Some((best_repl, best_len, _, _, _, _, _)) => {
                                    score < (*best_repl, *best_len)
                                }
                            };
                            if should_replace {
                                best_result = Some((
                                    replacement_count,
                                    len,
                                    decoded,
                                    repeat,
                                    frame_ms,
                                    strength,
                                    sample_rate,
                                ));
                            }
                            println!(
                                "Decode produced low-confidence result ({} bytes, {} replacement chars); continuing search.",
                                len, replacement_count
                            );
                        }
                        Err(_) => {
                            println!(
                                "Attempt failed for repeat={}, frame={} ms.",
                                repeat, frame_ms
                            );
                        }
                    }
                }
            }
        }
    }

    if let Some((_, len, decoded, repeat, frame_ms, strength, sample_rate)) = best_result {
        let sr_label_owned = sample_rate
            .map(|sr| sr.to_string())
            .unwrap_or_else(|| "auto".to_string());
        println!(
            "\nNo high-confidence decode found. Best attempt used repeat={}, frame={} ms, strength={}%, sample_rate_hint={}, yielding {} bytes:\n\"{}\"",
            repeat,
            frame_ms,
            strength,
            sr_label_owned,
            len,
            decoded.message
        );
    } else {
        println!("\nAll candidate combinations failed. Consider providing explicit configuration hints or adjusting SS_REPEAT/strength manually.");
    }
}

#[derive(Default)]
struct TempPathGuard(Option<PathBuf>);

impl TempPathGuard {
    fn set(&mut self, path: PathBuf) {
        self.0 = Some(path);
    }
}

impl Drop for TempPathGuard {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            if let Err(err) = fs::remove_file(&path) {
                eprintln!(
                    "Warning: failed to remove temporary file {}: {}",
                    path.display(),
                    err
                );
            }
        }
    }
}

struct CsvConversion {
    path: PathBuf,
    sample_rate: u32,
    samples: usize,
}

fn csv_to_temp_wav(csv_path: &Path) -> Result<CsvConversion, String> {
    let contents = fs::read_to_string(csv_path)
        .map_err(|err| format!("Failed to read CSV {}: {}", csv_path.display(), err))?;

    let mut samples: Vec<f32> = Vec::new();
    let mut prev_time: Option<f64> = None;
    let mut dt_acc = 0.0f64;
    let mut dt_count = 0usize;

    for (idx, line) in contents.lines().enumerate() {
        if idx == 0 {
            continue; // skip header
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut columns = line.split(',');
        let time_str = columns.next().unwrap_or("").trim();
        let value_str = columns.next().unwrap_or("").trim();

        if time_str.is_empty() || value_str.is_empty() {
            continue;
        }

        let time_val: f64 = match time_str.parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let sample_val: f64 = match value_str.parse() {
            Ok(v) => v,
            Err(_) => continue,
        };

        samples.push(sample_val as f32);
        if let Some(prev) = prev_time {
            let dt = time_val - prev;
            if dt.abs() > 1e-9 {
                dt_acc += dt.abs();
                dt_count += 1;
            }
        }
        prev_time = Some(time_val);
    }

    if samples.is_empty() {
        return Err(format!(
            "CSV {} did not contain parsable TIME/CH2 samples.",
            csv_path.display()
        ));
    }

    let avg_dt = if dt_count > 0 {
        dt_acc / dt_count as f64
    } else {
        1.0 / 8000.0
    };
    let sample_rate = ((1.0 / avg_dt).round().max(1.0)) as u32;

    let mut dc = 0.0f32;
    for sample in &samples {
        dc += *sample;
    }
    let mean = dc / samples.len() as f32;
    let mut peak = 0.0f32;
    for sample in &mut samples {
        *sample -= mean;
        peak = peak.max(sample.abs());
    }
    if peak > 0.0 {
        for sample in &mut samples {
            *sample /= peak;
        }
    }

    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let temp_dir = manifest.join("output_data").join("temp");
    fs::create_dir_all(&temp_dir).map_err(|err| {
        format!(
            "Failed to create temp directory {}: {}",
            temp_dir.display(),
            err
        )
    })?;

    let stem = csv_path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(sanitize_for_filename)
        .unwrap_or_else(|| "csv_waveform".to_string());
    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("System time error: {}", err))?
        .as_millis();
    let temp_path = temp_dir.join(format!("{}_{}_{}.wav", stem, process::id(), unique_suffix));

    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut writer = WavWriter::create(&temp_path, spec)
        .map_err(|err| format!("Failed to create temp WAV {}: {}", temp_path.display(), err))?;

    let sample_count = samples.len();
    for sample in samples.into_iter() {
        let quantized = (sample.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        writer
            .write_sample(quantized)
            .map_err(|err| format!("Failed to write sample to {}: {}", temp_path.display(), err))?;
    }
    writer
        .finalize()
        .map_err(|err| format!("Failed to finalize WAV {}: {}", temp_path.display(), err))?;

    Ok(CsvConversion {
        path: temp_path,
        sample_rate,
        samples: sample_count,
    })
}

fn sanitize_for_filename(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if matches!(ch, '-' | '_') {
            out.push(ch);
        } else if ch.is_whitespace() {
            if !out.ends_with('_') {
                out.push('_');
            }
        } else {
            if !out.ends_with('_') {
                out.push('_');
            }
        }
    }
    if out.is_empty() {
        out.push_str("temp");
    }
    out
}

fn is_csv_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("csv"))
        .unwrap_or(false)
}

fn read_wav_sample_rate(path: &Path) -> Option<u32> {
    WavReader::open(path)
        .map(|reader| reader.spec().sample_rate)
        .ok()
}
