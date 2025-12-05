use hound::{WavReader, WavWriter};
use realfft::RealFftPlanner;
use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};

// =============================================================================
// CONSTANTS - Watermark configuration
// =============================================================================

// Pilot pattern: A known sequence at the start to help decoder find the threshold
// Alternating 0s and 1s give us clear separation between high and low magnitudes
pub const PILOT_PATTERN: [u8; 8] = [0, 1, 0, 1, 0, 1, 0, 1];

const START_BIN: usize = 48; // embed starting away from low frequencies to reduce audibility

// Sample normalization divisor for i16 -> f32 conversion
const SAMPLE_DIVISOR: f32 = 32768.0;

const SAMPLE_RATES: [u32; 3] = [8000, 16_000, 32_000];
const FRAME_DURATIONS_MS: [u32; 3] = [20, 32, 64];
const WATERMARK_STRENGTHS: [u32; 4] = [5, 15, 30, 50];

// Input and output file paths
const INPUT_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/input_data/OSR_us_000_0057_8k.wav"
);
const OUTPUT_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/output_data/OSR_us_000_0057_8k_watermarked.wav"
);

// =============================================================================
// ORCHESTRATOR: Main entry point that coordinates the encoding pipeline
// =============================================================================

/// Visualization data for encoding
#[derive(Clone)]
pub struct EncodeVisualization {
    pub original_frame: Vec<f32>,
    pub watermarked_frame: Vec<f32>,
    pub bit_sequence: Vec<u8>,
}

/// WASM-compatible encoder that accepts audio samples directly
/// Returns encoded samples as Vec<f32>
pub fn encode_audio_samples(
    samples: &[f32],
    sample_rate: u32,
    message: &str,
    frame_duration_ms: u32,
    strength_percent: u32,
) -> Vec<f32> {
    let (encoded, _) = encode_audio_samples_with_viz(samples, sample_rate, message, frame_duration_ms, strength_percent);
    encoded
}

/// WASM-compatible encoder that returns both encoded samples and visualization data
pub fn encode_audio_samples_with_viz(
    samples: &[f32],
    sample_rate: u32,
    message: &str,
    frame_duration_ms: u32,
    strength_percent: u32,
) -> (Vec<f32>, EncodeVisualization) {
    let strength_percent = strength_percent.max(15); // enforce a floor so the watermark survives noisy audio

    // Build the bit sequence (pilot + length + message)
    let bits = build_bit_sequence(message);

    // Calculate frame length
    let frame_len = frame_length_samples(sample_rate, frame_duration_ms);
    if frame_len <= START_BIN {
        // Return original samples if frame length is too small
        let empty_viz = EncodeVisualization {
            original_frame: Vec::new(),
            watermarked_frame: Vec::new(),
            bit_sequence: bits,
        };
        return (samples.to_vec(), empty_viz);
    }

    // Extract first frame for visualization
    let first_frame_original: Vec<f32> = samples.iter().take(frame_len).copied().collect();

    // Convert strength percentage to fraction
    // Keep the watermark subtle: scale more gently so it remains inaudible.
    let strength = (strength_percent as f32 / 30.0).min(0.5);

    // Embed watermark into audio via FFT processing
    let encoded = embed_watermark_fft(samples, &bits, frame_len, strength);
    
    // Extract first frame of watermarked audio for visualization
    let first_frame_watermarked: Vec<f32> = encoded.iter().take(frame_len).copied().collect();

    let viz = EncodeVisualization {
        original_frame: first_frame_original,
        watermarked_frame: first_frame_watermarked,
        bit_sequence: bits,
    };

    (encoded, viz)
}

pub fn encode_sample(message: &str) {
    // Step 1: Load audio and get normalized samples + metadata
    let (base_samples, base_spec) = load_and_normalize_audio(Path::new(INPUT_PATH));

    // Step 2: Build the bit sequence (pilot + length + message)
    let bits = build_bit_sequence(message);

    // Step 3: Iterate through experiment grid and emit each combination
    for &target_rate in SAMPLE_RATES.iter() {
        let samples_for_rate: Cow<[f32]> = if target_rate == base_spec.sample_rate {
            Cow::Borrowed(base_samples.as_slice())
        } else {
            Cow::Owned(resample_audio(
                base_samples.as_slice(),
                base_spec.sample_rate,
                target_rate,
            ))
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
                let strength = (strength_percent.max(15) as f32 / 30.0).min(0.5);

                // Step 3: Embed bits into audio via FFT processing
                let encoded =
                    embed_watermark_fft(samples_for_rate.as_ref(), &bits, frame_len, strength);

                // Step 4: Convert back to i16 samples
                let quantized = quantize_to_i16(encoded);

                // Step 5: Write the watermarked audio to disk
                let output_path = experiment_output_path(target_rate, frame_ms, strength_percent);
                write_wav_file(&output_path, &quantized, spec_for_rate.clone());

                if target_rate == base_spec.sample_rate && frame_ms == 32 && strength_percent == 15
                {
                    // Maintain legacy output for decoder convenience
                    write_wav_file(Path::new(OUTPUT_PATH), &quantized, spec_for_rate.clone());
                }
            }
        }
    }
}

// =============================================================================
// STEP 1: Load and normalize audio
// =============================================================================

fn load_and_normalize_audio(input_path: &Path) -> (Vec<f32>, hound::WavSpec) {
    println!("Loading clean audio from {}", input_path.display());

    let mut reader = WavReader::open(input_path).expect("failed to open wav file");

    // Read and normalize samples in a single pass: i16 -> f32 in [-1.0, 1.0]
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

// =============================================================================
// STEP 2: Build bit sequence (pilot + length + message)
// =============================================================================

fn build_bit_sequence(message: &str) -> Vec<u8> {
    let message_bytes = message.as_bytes();
    let length_header = message_bytes.len() as u16;

    let mut bits = Vec::new();

    // 1. Pilot pattern for threshold calibration
    bits.extend_from_slice(&PILOT_PATTERN);

    // 2. Length header (16 bits, MSB first)
    for shift in (0..16).rev() {
        bits.push(((length_header >> shift) & 1) as u8);
    }

    // Position:  15 14 13 12 11 10  9  8  7  6  5  4  3  2  1  0
    // Binary:     0  0  0  0  0  0  0  0  0  0  0  0  0  1  0  1

    // 3. Message payload (8 bits per byte, MSB first)
    for &byte in message_bytes {
        for shift in (0..8).rev() {
            bits.push((byte >> shift) & 1);
        }
    }

    println!(
        "Encoding message {:?} ({} bytes)",
        message,
        message_bytes.len()
    );
    println!(
        "Total bits to embed (pilot + length + data): {}",
        bits.len()
    );

    bits
}

// =============================================================================
// STEP 3: Embed watermark using FFT
// =============================================================================

fn embed_watermark_fft(audio: &[f32], bits: &[u8], frame_len: usize, strength: f32) -> Vec<f32> {
    // Use next_power_of_two to match decoder's FFT size
    let fft_len = frame_len.next_power_of_two().max(2);
    
    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(fft_len);
    let ifft = planner.plan_fft_inverse(fft_len);

    let mut buffer = vec![0.0f32; fft_len];

    //buffer (256 slots):
    //[___|___|___|___|___| ... |___|___|___]

    let mut spectrum = fft.make_output_vec();
    let mut output = Vec::new();

    if START_BIN >= spectrum.len() {
        return audio.to_vec();
    }

    // Process each frame
    for chunk in audio.chunks(frame_len) {
        // Load audio
        buffer.fill(0.0); // wipe clean every time because multiple iterations
        buffer[..chunk.len()].copy_from_slice(chunk); //copies chunk into our empty slots

        // Time → Frequency
        fft.process(&mut buffer, &mut spectrum).expect("FFT failed"); //i will explain in the decoder video

        // Embed bits: boost (1.15) or reduce (0.85) frequency amplitudes
        // Produces: &0, &1, &0, &1, &0, &1, ...
        // (references to each bit)
        // Same as spectrum[10..129]
        // Includes: spectrum[10], spectrum[11], spectrum[12], ..., spectrum[128]
        // That's 119 elements

        //  Left side:     Right side:
        // &0     ←──→  bin10
        // &1     ←──→  bin11
        // &0     ←──→  bin12
        // &1     ←──→  bin13
        // ...
        for (&bit, bin) in bits.iter().zip(&mut spectrum[START_BIN..]) {
            let scale = if bit == 1 {
                1.0 + strength
            } else {
                (1.0 - strength).max(0.0)
            };
            bin.re *= scale;
            bin.im *= scale;
        }

        // Frequency → Time
        ifft.process(&mut spectrum, &mut buffer)
            .expect("IFFT failed");

        // Normalize and append
        output.extend(buffer[..chunk.len()].iter().map(|x| x / fft_len as f32));
    }

    output
}

// =============================================================================
// STEP 4: Quantize to i16 samples
// =============================================================================

fn quantize_to_i16(encoded: Vec<f32>) -> Vec<i16> {
    encoded
        .into_iter()
        .map(|sample| (sample.clamp(-1.0, 1.0) * 32767.0).round() as i16)
        .collect()
}

// =============================================================================
// STEP 5: Write WAV file to disk
// =============================================================================

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
