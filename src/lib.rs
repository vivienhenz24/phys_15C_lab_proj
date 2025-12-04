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

/// Struct to hold decoding visualization data for JS
#[derive(Serialize, Deserialize)]
pub struct DecodeVisualizationResult {
    pub bit_sequence: Vec<u8>,
    pub scores: Vec<f32>,
    pub votes: Vec<f32>,
    pub threshold: f32,
    pub avg_high: f32,
    pub avg_low: f32,
    pub inverted: bool,
    pub first_frame: Vec<f32>,
}

/// Struct to hold decoding result with visualization data
#[derive(Serialize, Deserialize)]
pub struct DecodeResult {
    pub message: String,
    pub raw_bytes: Vec<u8>,
    pub visualization: DecodeVisualizationResult,
}

/// Struct to hold encoding visualization data for JS
#[derive(Serialize, Deserialize)]
pub struct EncodeVisualizationResult {
    pub original_frame: Vec<f32>,
    pub watermarked_frame: Vec<f32>,
    pub bit_sequence: Vec<u8>,
}

/// Struct to hold encoding result with visualization data
#[derive(Serialize, Deserialize)]
pub struct EncodeResult {
    pub encoded_samples: Vec<f32>,
    pub visualization: EncodeVisualizationResult,
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

/// Encode a message into audio samples with visualization data
/// 
/// # Arguments
/// * `samples` - Audio samples as f32 array (normalized to [-1.0, 1.0])
/// * `sample_rate` - Sample rate in Hz
/// * `message` - Message string to encode
/// * `frame_duration_ms` - Frame duration in milliseconds (default: 32)
/// * `strength_percent` - Watermark strength as percentage (default: 15)
/// 
/// # Returns
/// JSON string containing encoded samples and visualization data
#[wasm_bindgen]
pub fn encode_audio_with_viz(
    samples: Vec<f32>,
    sample_rate: u32,
    message: String,
    frame_duration_ms: u32,
    strength_percent: u32,
) -> String {
    let (encoded_samples, viz) = encoder::encode_audio_samples_with_viz(
        &samples,
        sample_rate,
        &message,
        frame_duration_ms,
        strength_percent,
    );
    
    let result = EncodeResult {
        encoded_samples,
        visualization: EncodeVisualizationResult {
            original_frame: viz.original_frame,
            watermarked_frame: viz.watermarked_frame,
            bit_sequence: viz.bit_sequence,
        },
    };
    
    serde_json::to_string(&result).unwrap()
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

/// Decode a message from audio samples with visualization data
/// 
/// # Arguments
/// * `samples` - Audio samples as f32 array (normalized to [-1.0, 1.0])
/// * `sample_rate` - Sample rate in Hz
/// 
/// # Returns
/// JSON string containing decoded message and visualization data
#[wasm_bindgen]
pub fn decode_audio_with_viz(samples: Vec<f32>, sample_rate: u32) -> String {
    let (decoded, viz) = decoder::decode_audio_samples_with_viz(&samples, sample_rate);
    let result = DecodeResult {
        message: decoded.message,
        raw_bytes: decoded.raw_bytes,
        visualization: DecodeVisualizationResult {
            bit_sequence: viz.bit_sequence,
            scores: viz.scores,
            votes: viz.votes,
            threshold: viz.threshold,
            avg_high: viz.avg_high,
            avg_low: viz.avg_low,
            inverted: viz.inverted,
            first_frame: viz.first_frame,
        },
    };
    serde_json::to_string(&result).unwrap()
}

