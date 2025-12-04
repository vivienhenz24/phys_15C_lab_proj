pub mod decoder;
pub mod encoder;

use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};

// Re-export the encoder and decoder modules
pub use decoder::DecodedWatermark;
pub use encoder::PILOT_PATTERN;

/// Struct to hold decoded watermark data for JS
#[derive(Serialize, Deserialize)]
pub struct DecodedResult {
    pub message: String,
    pub raw_bytes: Vec<u8>,
}

/// Encode a message into audio samples
/// 
/// # Arguments
/// * `samples` - Audio samples as f32 array (normalized to [-1.0, 1.0])
/// * `sample_rate` - Sample rate in Hz
/// * `message` - Message string to encode
/// * `frame_duration_ms` - Frame duration in milliseconds (default: 32)
/// * `strength_percent` - Watermark strength as percentage (default: 15)
/// 
/// # Returns
/// Encoded audio samples as Vec<f32>
#[wasm_bindgen]
pub fn encode_audio(
    samples: Vec<f32>,
    sample_rate: u32,
    message: String,
    frame_duration_ms: u32,
    strength_percent: u32,
) -> Vec<f32> {
    encoder::encode_audio_samples(
        &samples,
        sample_rate,
        &message,
        frame_duration_ms,
        strength_percent,
    )
}

/// Decode a message from audio samples
/// 
/// # Arguments
/// * `samples` - Audio samples as f32 array (normalized to [-1.0, 1.0])
/// * `sample_rate` - Sample rate in Hz
/// 
/// # Returns
/// Decoded watermark containing the message and raw bytes as JSON string
#[wasm_bindgen]
pub fn decode_audio(samples: Vec<f32>, sample_rate: u32) -> String {
    let result = decoder::decode_audio_samples(&samples, sample_rate);
    let decoded_result = DecodedResult {
        message: result.message,
        raw_bytes: result.raw_bytes,
    };
    serde_json::to_string(&decoded_result).unwrap()
}

