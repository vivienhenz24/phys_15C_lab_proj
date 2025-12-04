import init, { encode_audio, decode_audio, encode_audio_with_viz, decode_audio_with_viz } from '../pkg/msg_encoder';

let wasmInitialized = false;

export async function initWasm(): Promise<void> {
  if (!wasmInitialized) {
    await init();
    wasmInitialized = true;
  }
}

export interface DecodedResult {
  message: string;
  raw_bytes: number[];
}

export interface EncodeVisualizationResult {
  original_frame: number[];
  watermarked_frame: number[];
  bit_sequence: number[];
}

export interface EncodeResult {
  encoded_samples: number[];
  visualization: EncodeVisualizationResult;
}

export interface DecodeVisualizationResult {
  bit_sequence: number[];
  scores: number[];
  votes: number[];
  threshold: number;
  avg_high: number;
  avg_low: number;
  inverted: boolean;
  first_frame: number[];
}

export interface DecodeResult {
  message: string;
  raw_bytes: number[];
  visualization: DecodeVisualizationResult;
}

export async function encodeAudio(
  samples: Float32Array,
  sampleRate: number,
  message: string,
  frameDurationMs: number = 32,
  strengthPercent: number = 15
): Promise<Float32Array> {
  await initWasm();
  const encoded = encode_audio(samples, sampleRate, message, frameDurationMs, strengthPercent);
  return encoded;
}

export async function encodeAudioWithViz(
  samples: Float32Array,
  sampleRate: number,
  message: string,
  frameDurationMs: number = 32,
  strengthPercent: number = 15
): Promise<EncodeResult> {
  await initWasm();
  const resultJson = encode_audio_with_viz(samples, sampleRate, message, frameDurationMs, strengthPercent);
  return JSON.parse(resultJson) as EncodeResult;
}

export async function decodeAudio(
  samples: Float32Array,
  sampleRate: number
): Promise<DecodedResult> {
  await initWasm();
  const resultJson = decode_audio(samples, sampleRate);
  return JSON.parse(resultJson) as DecodedResult;
}

export async function decodeAudioWithViz(
  samples: Float32Array,
  sampleRate: number
): Promise<DecodeResult> {
  await initWasm();
  const resultJson = decode_audio_with_viz(samples, sampleRate);
  return JSON.parse(resultJson) as DecodeResult;
}

