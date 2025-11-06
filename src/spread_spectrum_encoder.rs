use hound::{WavReader, WavWriter};
use realfft::RealFftPlanner;
use std::fs;
use std::path::{Path, PathBuf};

// =============================================================================
// Spread-Spectrum Encoder
// =============================================================================

const START_BIN: usize = 10;
const SAMPLE_DIVISOR: f32 = 32768.0;
const DESIRED_BAND_LEN: usize = 64; // fixed band width for robustness
const REPEAT_FACTOR: usize = 3; // frames per bit
const BAND_OFFSET: usize = 4; // skip a few bins near START_BIN

const SAMPLE_RATES: [u32; 4] = [8000, 16_000, 32_000, 48_000];
const FRAME_DURATIONS_MS: [u32; 3] = [20, 32, 64];
const WATERMARK_STRENGTHS: [u32; 4] = [5, 15, 30, 50];

const INPUT_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/input_data/OSR_us_000_0057_8k.wav"
);
const OUTPUT_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/output_data/spectrum/OSR_us_000_0057_8k_watermarked.wav"
);

pub fn encode_spread_spectrum(message: &str) {
    let (base_samples, base_spec) = load_and_normalize_audio(Path::new(INPUT_PATH));

    let bits = build_bit_sequence(message);

    for &target_rate in SAMPLE_RATES.iter() {
        let samples_for_rate = if target_rate == base_spec.sample_rate {
            base_samples.clone()
        } else {
            resample_audio(base_samples.as_slice(), base_spec.sample_rate, target_rate)
        };

        let mut spec_for_rate = base_spec.clone();
        spec_for_rate.sample_rate = target_rate;

        for &frame_ms in FRAME_DURATIONS_MS.iter() {
            let frame_len = frame_length_samples(target_rate, frame_ms);
            if frame_len <= START_BIN {
                println!(
                    "Skipping configuration {} Hz / {} ms: frame length too small",
                    target_rate, frame_ms
                );
                continue;
            }

            for &strength_percent in WATERMARK_STRENGTHS.iter() {
                let strength = (strength_percent as f32 / 100.0) * 7.0; // boost for spread-spectrum

                let encoded =
                    embed_watermark_spread(samples_for_rate.as_slice(), &bits, frame_len, strength);

                let quantized = quantize_to_i16(encoded);

                let output_path = experiment_output_path(target_rate, frame_ms, strength_percent);
                write_wav_file(&output_path, &quantized, spec_for_rate.clone());

                if target_rate == base_spec.sample_rate && frame_ms == 32 && strength_percent == 15
                {
                    write_wav_file(Path::new(OUTPUT_PATH), &quantized, spec_for_rate.clone());
                }
            }
        }
    }
}

fn embed_watermark_spread(audio: &[f32], bits: &[u8], frame_len: usize, strength: f32) -> Vec<f32> {
    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(frame_len);
    let ifft = planner.plan_fft_inverse(frame_len);

    let mut buffer = vec![0.0f32; frame_len];
    let mut spectrum = fft.make_output_vec();
    let mut output = Vec::with_capacity(audio.len());

    if START_BIN >= spectrum.len() {
        return audio.to_vec();
    }

    let max_band = DESIRED_BAND_LEN.max(1);

    let mut frame_index = 0usize;
    for chunk in audio.chunks(frame_len) {
        buffer.fill(0.0);
        buffer[..chunk.len()].copy_from_slice(chunk);

        fft.process(&mut buffer, &mut spectrum).expect("FFT failed");

        let bit_index = frame_index / REPEAT_FACTOR;
        if bit_index < bits.len() {
            let band_len = spectrum
                .len()
                .saturating_sub(START_BIN + BAND_OFFSET)
                .min(max_band)
                .max(1);

            // Use deterministic PN sequence for this bit across the band
            let sign = if bits[bit_index] == 1 { 1.0 } else { -1.0 };

            // Energy-normalized scaling across band
            let scale_per_bin = strength / (band_len as f32).sqrt();

            for k in 0..band_len {
                let idx = START_BIN + BAND_OFFSET + k;
                if idx >= spectrum.len() {
                    break;
                }
                let pn = pn_value(bit_index as u32, k as u32);
                let scale = 1.0 + sign * pn * scale_per_bin;
                spectrum[idx].re *= scale;
                spectrum[idx].im *= scale;
            }

            // keep same bit for next REPEAT_FACTOR-1 frames
        }

        ifft.process(&mut spectrum, &mut buffer)
            .expect("IFFT failed");
        output.extend(buffer[..chunk.len()].iter().map(|x| x / frame_len as f32));
        frame_index += 1;
    }

    output
}

#[inline]
fn pn_value(bit_seed: u32, tap: u32) -> f32 {
    // Simple xorshift-based PRNG mapped to {-1.0, 1.0}
    let mut x = bit_seed
        .wrapping_mul(0x9E3779B9)
        .wrapping_add(tap.wrapping_mul(0x85EBCA6B));
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    if (x & 1) == 0 {
        1.0
    } else {
        -1.0
    }
}

fn load_and_normalize_audio(input_path: &Path) -> (Vec<f32>, hound::WavSpec) {
    println!("Loading clean audio from {}", input_path.display());

    let mut reader = WavReader::open(input_path).expect("failed to open wav file");
    let mut normalized: Vec<f32> = Vec::new();
    for sample_result in reader.samples::<i16>() {
        let sample = sample_result.expect("failed to open sound file");
        let normalized_sample = (sample as f32) / SAMPLE_DIVISOR;
        normalized.push(normalized_sample);
    }
    let spec = reader.spec();
    println!(
        "Read and normalized {} samples at {} Hz",
        normalized.len(),
        spec.sample_rate
    );
    (normalized, spec)
}

fn build_bit_sequence(message: &str) -> Vec<u8> {
    let message_bytes = message.as_bytes();
    let length_header = message_bytes.len() as u16;

    let mut bits = Vec::new();

    // Pilot pattern for basic thresholding (short)
    const PILOT: [u8; 8] = [0, 1, 0, 1, 0, 1, 0, 1];
    bits.extend_from_slice(&PILOT);

    for shift in (0..16).rev() {
        bits.push(((length_header >> shift) & 1) as u8);
    }

    for &byte in message_bytes {
        for shift in (0..8).rev() {
            bits.push((byte >> shift) & 1);
        }
    }

    println!(
        "Encoding (spread-spectrum) message {:?} ({} bytes), total bits {}",
        message,
        message_bytes.len(),
        bits.len()
    );

    bits
}

fn quantize_to_i16(encoded: Vec<f32>) -> Vec<i16> {
    encoded
        .into_iter()
        .map(|sample| (sample.clamp(-1.0, 1.0) * 32767.0).round() as i16)
        .collect()
}

fn write_wav_file(output_path: &Path, quantized: &[i16], spec: hound::WavSpec) {
    if let Some(parent) = output_path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            panic!(
                "failed to create output directory {}: {err}",
                parent.display()
            );
        }
    }

    let mut writer = WavWriter::create(output_path, spec).expect("failed to create wav writer");
    for &sample in quantized {
        writer.write_sample(sample).expect("failed to write sample");
    }
    writer.finalize().expect("failed to finalize wav file");
    println!("Wrote watermarked audio to {}", output_path.display());
}

fn experiment_output_path(sample_rate: u32, frame_ms: u32, strength_percent: u32) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("output_data")
        .join("spectrum")
        .join(format!("{sample_rate}_{frame_ms}_{strength_percent}.wav"))
}

fn frame_length_samples(sample_rate: u32, frame_ms: u32) -> usize {
    (((sample_rate as f32) * (frame_ms as f32) / 1000.0).round() as usize).max(1)
}

fn resample_audio(samples: &[f32], original_rate: u32, target_rate: u32) -> Vec<f32> {
    if samples.is_empty() || original_rate == target_rate {
        return samples.to_vec();
    }

    let ratio = target_rate as f32 / original_rate as f32;
    let new_len = ((samples.len() as f32) * ratio).ceil() as usize;
    let mut output = Vec::with_capacity(new_len);

    for idx in 0..new_len {
        let src_pos = idx as f32 / ratio;
        let base = src_pos.floor() as usize;
        let frac = src_pos - base as f32;

        if base + 1 < samples.len() {
            let start = samples[base];
            let end = samples[base + 1];
            output.push(start + (end - start) * frac);
        } else if let Some(&last) = samples.last() {
            output.push(last);
        }
    }

    output
}
