use std::cmp::Ordering; // for median selection
use std::path::{Path, PathBuf}; // build file paths

use hound::WavReader; // read WAV data
use realfft::RealFftPlanner; // perform FFTs

// --- Decoder configuration mirroring the encoder ---
const PILOT_PATTERN: [u8; 8] = [0, 1, 0, 1, 0, 1, 0, 1]; // known pilot
const LENGTH_HEADER_BITS: usize = 16; // payload length field
const WATERMARK_FRAME_DURATION: f32 = 0.032; // frame duration (32ms)
const SAMPLE_DIVISOR: f32 = 32768.0; // i16 -> f32 scale
const START_BIN: usize = 10; // first watermark bin

/// Struct returned by the decoder.
pub struct DecodedWatermark {
    pub message: String,    // recovered UTF-8 text
    pub raw_bytes: Vec<u8>, // raw byte payload
}

/// WASM-compatible decoder that accepts audio samples directly
pub fn decode_audio_samples(samples: &[f32], sample_rate: u32) -> DecodedWatermark {
    let (scores, votes, _valid, _skipped) = summarise_frames(samples, sample_rate, 3); // aggregate frame stats

    if scores.len() < PILOT_PATTERN.len() + LENGTH_HEADER_BITS {
        // Return empty result if not enough bins
        return DecodedWatermark {
            message: String::new(),
            raw_bytes: Vec::new(),
        };
    }

    let (avg_high, avg_low, threshold) = pilot_stats(&scores); // global threshold from pilot

    let bits = decide_bits(&scores, &votes, threshold, avg_high, avg_low); // convert scores to bits

    let (_pilot_bits, remainder) = bits.split_at(PILOT_PATTERN.len()); // separate pilot

    let (len_bits, data_bits_all) = remainder.split_at(LENGTH_HEADER_BITS.min(remainder.len())); // length header slice
    let message_bytes = decode_length_header(len_bits); // parse payload size

    let required_bits = message_bytes.saturating_mul(8); // bits required
    let available_bits = data_bits_all.len(); // bits available
    let actual_bits = required_bits.min(available_bits); // clamp

    let message_bits = data_bits_all[..actual_bits].to_vec(); // payload bits

    bits_to_message(message_bits) // convert to DecodedWatermark
}

/// Blindly decode the watermark from the provided path.
pub fn decode_watermarked_sample(path: impl AsRef<Path>) -> DecodedWatermark {
    println!("=== Audio Watermark Decoder (Blind) ===\n"); // header

    let (samples, sample_rate) = load_audio(path.as_ref()); // load waveform
    let decoded = decode_audio_samples(&samples, sample_rate);
    
    println!(
        "\nDecoded message: \"{}\" (bytes: {:?})",
        decoded.message, decoded.raw_bytes
    );
    println!("\n=== Decoding Complete ==="); // footer
    decoded
}

/// Convenience path used by CLI.
pub fn default_watermarked_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("output_data")
        .join("OSR_us_000_0057_8k_watermarked.wav")
}

// --- Frame analysis helpers -------------------------------------------------

fn summarise_frames(
    samples: &[f32],
    sample_rate: u32,
    window_radius: usize,
) -> (Vec<f32>, Vec<f32>, usize, usize) {
    let frame_len = ((sample_rate as f32 * WATERMARK_FRAME_DURATION)
        .round()
        .max(1.0)) as usize; // samples per frame
    let fft_len = frame_len.next_power_of_two().max(2); // FFT size

    let mut planner = RealFftPlanner::<f32>::new(); // FFT planner
    let forward = planner.plan_fft_forward(fft_len); // forward FFT
    let mut scratch = forward.make_scratch_vec(); // scratch buffer
    let mut buffer = vec![0.0f32; fft_len]; // time-domain buffer
    let mut spectrum = forward.make_output_vec(); // frequency-domain buffer

    let usable_bins = spectrum.len().saturating_sub(START_BIN); // candidate bins
    let mut score_samples: Vec<Vec<f32>> =
        (0..usable_bins).map(|_| Vec::with_capacity(128)).collect(); // per-bin scores
    let mut vote_counts = vec![0u32; usable_bins]; // per-bin “1” votes
    let mut valid_frames = 0usize; // accepted frames
    let mut skipped_frames = 0usize; // rejected frames

    let mut offset = 0usize; // frame pointer
    while offset < samples.len() {
        let end = (offset + frame_len).min(samples.len()); // clamp frame
        let frame = &samples[offset..end]; // frame view

        buffer.fill(0.0); // clear buffer
        buffer[..frame.len()].copy_from_slice(frame); // copy samples

        forward
            .process_with_scratch(&mut buffer, &mut spectrum, &mut scratch)
            .expect("FFT failed"); // FFT

        let mut magnitudes = Vec::with_capacity(usable_bins); // magnitude list
        for idx in 0..usable_bins {
            let bin = START_BIN + idx; // actual bin
            if bin >= spectrum.len() {
                break;
            }
            magnitudes.push(spectrum[bin].norm()); // magnitude
        }

        if magnitudes.len() < PILOT_PATTERN.len() {
            skipped_frames += 1; // not enough bins
            offset += frame_len;
            continue;
        }

        let scores = spectral_scores(&magnitudes, window_radius); // log-normalised scores
        if let Some((threshold, matches)) = frame_pilot_stats(&scores) {
            if matches >= 5 {
                valid_frames += 1; // accept frame
                for (idx, score) in scores.iter().enumerate() {
                    if idx >= usable_bins {
                        break;
                    }
                    score_samples[idx].push(*score); // record score
                    if *score >= threshold {
                        vote_counts[idx] += 1; // vote for “1”
                    }
                }
            } else {
                skipped_frames += 1; // pilot mismatch
            }
        } else {
            skipped_frames += 1; // pilot unusable
        }

        offset += frame_len; // advance frame pointer
    }

    if valid_frames == 0 {
        panic!("unable to decode watermark: no reliable frames detected");
    }

    let mut medians = Vec::with_capacity(usable_bins); // aggregated scores
    for mut samples in score_samples {
        if samples.is_empty() {
            medians.push(0.0); // default
        } else {
            let mid = samples.len() / 2; // median index
            let (_, median, _) = samples
                .select_nth_unstable_by(mid, |a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal)); // median selection
            medians.push(*median);
        }
    }

    let ratios = vote_counts
        .into_iter()
        .map(|votes| votes as f32 / valid_frames as f32)
        .collect(); // convert to ratios

    (medians, ratios, valid_frames, skipped_frames) // summary
}

fn spectral_scores(magnitudes: &[f32], window_radius: usize) -> Vec<f32> {
    let epsilon = 1e-12f32; // avoid log(0)
    let log_mags: Vec<f32> = magnitudes.iter().map(|&v| v.max(epsilon).ln()).collect(); // log spectrum

    let mut prefix = vec![0f64; log_mags.len() + 1]; // prefix sums
    for (idx, &value) in log_mags.iter().enumerate() {
        prefix[idx + 1] = prefix[idx] + value as f64;
    }

    let mut scores = Vec::with_capacity(log_mags.len()); // output
    for (idx, &value) in log_mags.iter().enumerate() {
        let start = idx.saturating_sub(window_radius);
        let end = (idx + window_radius + 1).min(log_mags.len());
        let neighbours = end.saturating_sub(start + 1); // exclude self
        if neighbours == 0 {
            scores.push(0.0);
            continue;
        }
        let baseline = (prefix[end] - prefix[start] - value as f64) / neighbours.max(1) as f64; // neighbour average
        scores.push(value - baseline as f32); // relative score
    }

    scores
}

fn frame_pilot_stats(scores: &[f32]) -> Option<(f32, usize)> {
    if scores.len() < PILOT_PATTERN.len() {
        return None; // insufficient bins
    }

    let pilot = &scores[..PILOT_PATTERN.len()];
    let mut sum_high = 0.0f32;
    let mut sum_low = 0.0f32;
    let mut count_high = 0usize;
    let mut count_low = 0usize;

    for (score, expected) in pilot.iter().zip(PILOT_PATTERN.iter()) {
        if *expected == 1 {
            sum_high += score;
            count_high += 1;
        } else {
            sum_low += score;
            count_low += 1;
        }
    }

    if count_high == 0 || count_low == 0 {
        return None; // degenerate
    }

    let threshold = (sum_high / count_high as f32 + sum_low / count_low as f32) * 0.5; // per-frame decision
    let matches = pilot
        .iter()
        .zip(PILOT_PATTERN.iter())
        .filter(|(score, expected)| u8::from(**score >= threshold) == **expected)
        .count(); // match count

    Some((threshold, matches))
}

fn pilot_stats(scores: &[f32]) -> (f32, f32, f32) {
    let pilot = &scores[..PILOT_PATTERN.len()];
    let mut sum_high = 0.0f32;
    let mut sum_low = 0.0f32;
    let mut count_high = 0usize;
    let mut count_low = 0usize;

    for (score, expected) in pilot.iter().zip(PILOT_PATTERN.iter()) {
        if *expected == 1 {
            sum_high += score;
            count_high += 1;
        } else {
            sum_low += score;
            count_low += 1;
        }
    }

    let avg_high = sum_high / count_high as f32;
    let avg_low = sum_low / count_low as f32;
    let threshold = (avg_high + avg_low) * 0.5;
    (avg_high, avg_low, threshold)
}

fn decide_bits(
    scores: &[f32],
    votes: &[f32],
    threshold: f32,
    avg_high: f32,
    avg_low: f32,
) -> Vec<u8> {
    let decision_band = (avg_high - avg_low) * 0.1; // hysteresis

    scores
        .iter()
        .zip(votes.iter())
        .enumerate()
        .map(|(idx, (&score, &ratio))| {
            let in_length_header =
                (PILOT_PATTERN.len()..PILOT_PATTERN.len() + LENGTH_HEADER_BITS).contains(&idx); // header segments
            let bit = if in_length_header {
                u8::from(ratio >= 0.54 && score >= threshold)
            } else if score >= threshold {
                1 // confident one
            } else if score <= threshold - decision_band {
                0 // confident zero
            } else {
                u8::from(ratio >= 0.48 || score >= threshold - decision_band * 0.5)
                // soft fallback
            };
            bit
        })
        .collect()
}

// --- Bitstream utilities ----------------------------------------------------

fn decode_length_header(bits: &[u8]) -> usize {
    let mut len = 0u16;
    for bit in bits {
        len = (len << 1) | u16::from(bit & 1); // shift & merge
    }
    len as usize
}

fn bits_to_message(bits: Vec<u8>) -> DecodedWatermark {
    let mut bytes = Vec::with_capacity((bits.len() + 7) / 8); // allocate
    for chunk in bits.chunks(8) {
        let mut byte = 0u8;
        for bit in chunk {
            byte = (byte << 1) | (bit & 1); // pack bits
        }
        bytes.push(byte);
    }
    DecodedWatermark {
        message: String::from_utf8_lossy(&bytes).into_owned(),
        raw_bytes: bytes,
    }
}

// --- Audio I/O --------------------------------------------------------------

fn load_audio(path: &Path) -> (Vec<f32>, u32) {
    println!("Loading watermarked audio from {}", path.display());
    let mut reader = WavReader::open(path).expect("failed to open watermarked wav");
    let spec = reader.spec();
    let samples: Vec<f32> = reader
        .samples::<i16>()
        .map(|s| s.expect("failed to read sample") as f32 / SAMPLE_DIVISOR)
        .collect();
    println!(
        "Loaded {} samples at {} Hz",
        samples.len(),
        spec.sample_rate
    );
    (samples, spec.sample_rate)
}
