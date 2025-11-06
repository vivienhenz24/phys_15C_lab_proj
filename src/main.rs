// Import the standard library's environment module for reading command-line arguments
use std::env;
use std::path::{Path, PathBuf};

// Import modules we defined in separate files
mod decoder; // Contains all decoding logic
mod encoder; // Contains all encoding logic
mod spread_spectrum_encoder; // Spread-spectrum encoder
mod spread_spectrum_decoder; // Spread-spectrum decoder

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
        _ => print_usage(),
    }
}

fn print_usage() {
    println!("usage:");
    println!("  cargo run -- encode");
    println!("  cargo run -- encode-ss");
    println!("  cargo run -- decode [filename]");
    println!("  cargo run -- decode-ss [filename]");
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
            let path = if Path::new(&file).is_absolute() {
                PathBuf::from(&file)
            } else {
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("output_data")
                    .join("spectrum")
                    .join(&file)
            };
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
