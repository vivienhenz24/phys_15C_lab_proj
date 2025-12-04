use msg_encoder::{encoder, decoder};

fn create_test_audio(duration_seconds: f32, sample_rate: u32) -> Vec<f32> {
    let num_samples = (duration_seconds * sample_rate as f32) as usize;
    let mut samples = Vec::with_capacity(num_samples);
    
    // Generate richer audio with multiple frequencies and noise
    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        
        // Mix of multiple frequencies to provide rich spectrum
        let signal = (2.0 * std::f32::consts::PI * 200.0 * t).sin() * 0.15
                   + (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.15
                   + (2.0 * std::f32::consts::PI * 880.0 * t).sin() * 0.10
                   + (2.0 * std::f32::consts::PI * 1320.0 * t).sin() * 0.08;
        
        // Add more substantial noise for realistic conditions
        let noise = ((i * 12345 + i * i) % 10000) as f32 / 10000.0 * 0.2 - 0.1;
        
        samples.push(signal + noise);
    }
    
    samples
}

#[test]
fn test_encode_decode_fourier() {
    let message = "fourier";
    let sample_rate = 8000;
    let audio = create_test_audio(2.0, sample_rate);
    
    // Encode
    let encoded = encoder::encode_audio_samples(&audio, sample_rate, message, 32, 30);
    
    // Decode
    let decoded = decoder::decode_audio_samples(&encoded, sample_rate);
    
    assert_eq!(decoded.message, message, "Failed to decode 'fourier'");
}

#[test]
fn test_encode_decode_hello() {
    let message = "hello";
    let sample_rate = 8000;
    let audio = create_test_audio(2.0, sample_rate);
    
    // Encode with 15% strength (lower to reduce bit errors)
    let encoded = encoder::encode_audio_samples(&audio, sample_rate, message, 32, 15);
    
    // Decode
    let decoded = decoder::decode_audio_samples(&encoded, sample_rate);
    
    assert_eq!(decoded.message, message, "Failed to decode 'hello'");
}

#[test]
fn test_encode_decode_mister() {
    let message = "mister";
    let sample_rate = 8000;
    let audio = create_test_audio(2.0, sample_rate);
    
    // Encode with 15% strength (lower to reduce bit errors)
    let encoded = encoder::encode_audio_samples(&audio, sample_rate, message, 32, 15);
    
    // Decode
    let decoded = decoder::decode_audio_samples(&encoded, sample_rate);
    
    assert_eq!(decoded.message, message, "Failed to decode 'mister'");
}

