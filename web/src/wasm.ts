import init, { encode_audio, decode_audio } from '../pkg/msg_encoder';

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

export async function decodeAudio(
  samples: Float32Array,
  sampleRate: number
): Promise<DecodedResult> {
  await initWasm();
  const resultJson = decode_audio(samples, sampleRate);
  return JSON.parse(resultJson) as DecodedResult;
}

