// Import the standard library's environment module for reading command-line arguments
use std::env;
use std::path::{Path, PathBuf};

// Import modules we defined in separate files
mod decoder; // Contains all decoding logic
mod encoder; // Contains all encoding logic

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
        "encode" => encoder::encode_sample("hi"),
        "decode" => {
            let (path, config) = decode_target(args.next());
            decoder::decode_watermarked_sample(path, config);
        }
        _ => print_usage(),
    }
}

fn print_usage() {
    println!("usage:");
    println!("  cargo run -- encode");
    println!("  cargo run -- decode [filename]");
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
