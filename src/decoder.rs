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

/// Visualization data for decoding
#[allow(dead_code)]
pub struct DecodeVisualization {
    pub bit_sequence: Vec<u8>,
    pub scores: Vec<f32>,
    pub votes: Vec<f32>,
    pub threshold: f32,
    pub avg_high: f32,
    pub avg_low: f32,
    pub inverted: bool,
    pub first_frame: Vec<f32>,
}

/// WASM-compatible decoder that accepts audio samples directly
pub fn decode_audio_samples(samples: &[f32], sample_rate: u32) -> DecodedWatermark {
    let (decoded, _) = decode_audio_samples_with_viz(samples, sample_rate);
    decoded
}

/// WASM-compatible decoder that returns both decoded watermark and visualization data
pub fn decode_audio_samples_with_viz(samples: &[f32], sample_rate: u32) -> (DecodedWatermark, DecodeVisualization) {
    // Extract first frame for visualization
    let frame_len = ((sample_rate as f32 * WATERMARK_FRAME_DURATION)
        .round()
        .max(1.0)) as usize;
    let first_frame: Vec<f32> = samples.iter().take(frame_len).copied().collect();

    let (scores, votes, _valid, _skipped, frames_inverted) =
        summarise_frames(samples, sample_rate, 3); // aggregate frame stats

    if scores.len() < PILOT_PATTERN.len() + LENGTH_HEADER_BITS {
        // Return empty result if not enough bins
        let empty_viz = DecodeVisualization {
            bit_sequence: Vec::new(),
            scores: Vec::new(),
            votes: Vec::new(),
            threshold: 0.0,
            avg_high: 0.0,
            avg_low: 0.0,
            inverted: false,
            first_frame,
        };
        return (DecodedWatermark {
            message: String::new(),
            raw_bytes: Vec::new(),
        }, empty_viz);
    }

    let (avg_high, avg_low, threshold) = pilot_stats(&scores); // global threshold from pilot
    let inverted = frames_inverted || avg_high < avg_low; // detect polarity flip (some audio can invert our boost/reduce)

    let bits = decide_bits(
        &scores,
        &votes,
        threshold,
        avg_high,
        avg_low,
        inverted,
    ); // convert scores to bits

    let (_pilot_bits, remainder) = bits.split_at(PILOT_PATTERN.len()); // separate pilot

    let (len_bits, data_bits_all) = remainder.split_at(LENGTH_HEADER_BITS.min(remainder.len())); // length header slice
    
    #[cfg(debug_assertions)]
    {
        let bits_str: String = len_bits.iter().map(|b| b.to_string()).collect::<Vec<_>>().join("");
        eprintln!("Length header bits: {}", bits_str);
        
        // Show scores for length header bits
        let len_start = PILOT_PATTERN.len();
        let len_end = len_start + LENGTH_HEADER_BITS;
        eprintln!("Length header scores and votes:");
        for (i, idx) in (len_start..len_end).enumerate() {
            eprintln!("  Bit {}: score={:.6}, vote={:.3}, decoded={}", 
                i, scores[idx], votes[idx], len_bits[i]);
        }
        eprintln!("  Threshold: {:.6}, avg_high: {:.6}, avg_low: {:.6}", 
            threshold, avg_high, avg_low);
        eprintln!("  Inverted polarity: {}", inverted);
    }
    
    #[cfg(target_arch = "wasm32")]
    {
        let bits_str: String = len_bits.iter().map(|b| b.to_string()).collect::<Vec<_>>().join("");
        web_sys::console::log_1(&format!("Length header bits: {}", bits_str).into());
    }
    
    let header_len = decode_length_header(len_bits); // parse payload size (hint only)
    let max_bytes = data_bits_all.len() / 8; // how many whole bytes we can possibly recover
    if max_bytes == 0 {
        let empty_viz = DecodeVisualization {
            bit_sequence: bits.clone(),
            scores: scores.clone(),
            votes: votes.clone(),
            threshold,
            avg_high,
            avg_low,
            inverted,
            first_frame,
        };
        return (DecodedWatermark {
            message: String::new(),
            raw_bytes: Vec::new(),
        }, empty_viz);
    }

    // Try every plausible length and pick the one that yields the most readable ASCII.
    let mut best: Option<(f32, DecodedWatermark)> = None;
    for candidate_len in 1..=max_bytes {
        let bit_count = candidate_len * 8;
        let message_bits = data_bits_all[..bit_count].to_vec();
        let candidate = bits_to_message(message_bits, candidate_len);

        let printable_ratio = candidate
            .raw_bytes
            .iter()
            .filter(|b| b.is_ascii_graphic() || b.is_ascii_whitespace())
            .count() as f32
            / candidate_len as f32;
        let proximity = 1.0
            / (1.0
                + (candidate_len as i32 - header_len as i32).abs() as f32);
        let score =
            printable_ratio * 2.0 + candidate_len as f32 * 0.05 + 0.1 * proximity; // prefer longer printable text

        if best.as_ref().map_or(true, |(s, _)| score > *s) {
            best = Some((score, candidate));
        }
    }

    let (_score, chosen) = best.expect("at least one candidate length exists");
    
    #[cfg(debug_assertions)]
    {
        eprintln!(
            "Decoded length hint: {}, chosen length: {}, printable score: {:.3}",
            header_len,
            chosen.raw_bytes.len(),
            chosen
                .raw_bytes
                .iter()
                .filter(|b| b.is_ascii_graphic() || b.is_ascii_whitespace())
                .count() as f32
                / chosen.raw_bytes.len().max(1) as f32
        );
        let data_start = PILOT_PATTERN.len() + LENGTH_HEADER_BITS;
        let bits_to_show = (chosen.raw_bytes.len() * 8).min(data_bits_all.len());
        if bits_to_show > 0 {
            eprintln!("First {} data bits (after pilot+length):", bits_to_show);
            for idx in 0..bits_to_show {
                let global_idx = data_start + idx;
                let bit = data_bits_all[idx];
                let vote = votes.get(global_idx).copied().unwrap_or(0.0);
                let score = scores.get(global_idx).copied().unwrap_or(0.0);
                eprintln!(
                    "  bit {:02} => {} (score={:.3}, vote={:.3})",
                    idx, bit, score, vote
                );
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        web_sys::console::log_1(
            &format!(
                "Decoded length hint: {}, chosen length: {}",
                header_len,
                chosen.raw_bytes.len()
            )
            .into(),
        );
    }

    // Create visualization data
    let viz = DecodeVisualization {
        bit_sequence: bits,
        scores: scores.clone(),
        votes: votes.clone(),
        threshold,
        avg_high,
        avg_low,
        inverted,
        first_frame,
    };
    
    (chosen, viz)
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
) -> (Vec<f32>, Vec<f32>, usize, usize, bool) {
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
    let mut inverted_frames = 0usize; // frames whose pilot indicates flipped polarity

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
        if let Some((threshold, matches, frame_inverted)) = frame_pilot_stats(&scores) {
            if matches >= 5 {
                valid_frames += 1; // accept frame
                if frame_inverted {
                    inverted_frames += 1;
                }
                for (idx, score) in scores.iter().enumerate() {
                    if idx >= usable_bins {
                        break;
                    }
                    score_samples[idx].push(*score); // record score
                    let vote_one = if frame_inverted {
                        *score <= threshold
                    } else {
                        *score >= threshold
                    };
                    if vote_one {
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

    let inverted = inverted_frames * 2 >= valid_frames.max(1); // majority of frames inverted?

    (medians, ratios, valid_frames, skipped_frames, inverted) // summary
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

fn frame_pilot_stats(scores: &[f32]) -> Option<(f32, usize, bool)> {
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

    // Evaluate both normal and inverted polarity; pick whichever matches pilot better.
    let matches_normal = pilot
        .iter()
        .zip(PILOT_PATTERN.iter())
        .filter(|(score, expected)| u8::from(**score >= threshold) == **expected)
        .count();
    let matches_inverted = pilot
        .iter()
        .zip(PILOT_PATTERN.iter())
        .filter(|(score, expected)| u8::from(**score <= threshold) == **expected)
        .count();

    if matches_inverted > matches_normal {
        Some((threshold, matches_inverted, true))
    } else {
        Some((threshold, matches_normal, false))
    }
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
    inverted: bool,
) -> Vec<u8> {
    let decision_band = (avg_high - avg_low).abs() * 0.1; // hysteresis

    // In some signals the boosted bins end up lower than the reduced ones (phase/energy quirks).
    // When that happens, treat scores below the threshold as "1" and flip vote ratios accordingly.

    scores
        .iter()
        .zip(votes.iter())
        .enumerate()
        .map(|(idx, (&score, &ratio))| {
            let effective_ratio = if inverted { 1.0 - ratio } else { ratio };
            let (bit_is_one, bit_is_zero, soft_cmp) = if inverted {
                (
                    score <= threshold,
                    score >= threshold + decision_band * 3.0,
                    score <= threshold + decision_band * 0.75,
                )
            } else {
                (
                    score >= threshold,
                    score <= threshold - decision_band * 3.0,
                    score >= threshold - decision_band * 0.75,
                )
            };

            let in_length_header =
                (PILOT_PATTERN.len()..PILOT_PATTERN.len() + LENGTH_HEADER_BITS).contains(&idx); // header segments
            let bit = if in_length_header {
                u8::from(effective_ratio >= 0.54 && bit_is_one)
            } else if bit_is_one {
                1 // confident one
            } else if bit_is_zero {
                0 // confident zero
            } else {
                u8::from(effective_ratio >= 0.45 || soft_cmp)
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

fn bits_to_message(bits: Vec<u8>, expected_bytes: usize) -> DecodedWatermark {
    let mut bytes = Vec::with_capacity(expected_bytes);
    
    // Process exactly expected_bytes worth of bits (8 bits per byte)
    let bits_to_process = (expected_bytes * 8).min(bits.len());
    
    for byte_idx in 0..expected_bytes {
        let bit_start = byte_idx * 8;
        if bit_start >= bits_to_process {
            break;
        }
        
        let mut byte = 0u8;
        for bit_idx in 0..8 {
            let bit_pos = bit_start + bit_idx;
            if bit_pos < bits_to_process {
                byte = (byte << 1) | (bits[bit_pos] & 1);
            } else {
                // Pad with zeros if we run out of bits
                byte = byte << 1;
            }
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
