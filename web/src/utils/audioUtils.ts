// WAV encoding/decoding utilities

/**
 * Convert AudioBuffer to Float32Array samples
 */
export function audioBufferToSamples(audioBuffer: AudioBuffer): Float32Array {
  // Get the first channel (mono)
  const channelData = audioBuffer.getChannelData(0);
  return channelData;
}

/**
 * Convert Float32Array samples and sample rate to WAV Blob
 */
export function samplesToWav(samples: Float32Array, sampleRate: number): Blob {
  // Convert Float32Array to Int16Array for WAV encoding
  const int16Samples = new Int16Array(samples.length);
  for (let i = 0; i < samples.length; i++) {
    // Clamp to [-1, 1] and convert to 16-bit integer
    const clamped = Math.max(-1, Math.min(1, samples[i]));
    int16Samples[i] = clamped < 0 ? clamped * 0x8000 : clamped * 0x7FFF;
  }

  // Create WAV file buffer
  const buffer = new ArrayBuffer(44 + int16Samples.length * 2);
  const view = new DataView(buffer);

  // WAV header
  const writeString = (offset: number, string: string) => {
    for (let i = 0; i < string.length; i++) {
      view.setUint8(offset + i, string.charCodeAt(i));
    }
  };

  // RIFF header
  writeString(0, 'RIFF');
  view.setUint32(4, 36 + int16Samples.length * 2, true);
  writeString(8, 'WAVE');

  // fmt chunk
  writeString(12, 'fmt ');
  view.setUint32(16, 16, true); // fmt chunk size
  view.setUint16(20, 1, true); // audio format (1 = PCM)
  view.setUint16(22, 1, true); // number of channels (1 = mono)
  view.setUint32(24, sampleRate, true); // sample rate
  view.setUint32(28, sampleRate * 2, true); // byte rate
  view.setUint16(32, 2, true); // block align
  view.setUint16(34, 16, true); // bits per sample

  // data chunk
  writeString(36, 'data');
  view.setUint32(40, int16Samples.length * 2, true);

  // Write audio data
  let offset = 44;
  for (let i = 0; i < int16Samples.length; i++) {
    view.setInt16(offset, int16Samples[i], true);
    offset += 2;
  }

  return new Blob([buffer], { type: 'audio/wav' });
}

/**
 * Parse uploaded WAV file to samples and sample rate
 */
export async function wavFileToSamples(file: File): Promise<{ samples: Float32Array; sampleRate: number }> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    
    reader.onload = (e) => {
      try {
        const arrayBuffer = e.target?.result as ArrayBuffer;
        const view = new DataView(arrayBuffer);

        // Read WAV header
        const readString = (offset: number, length: number): string => {
          let str = '';
          for (let i = 0; i < length; i++) {
            str += String.fromCharCode(view.getUint8(offset + i));
          }
          return str;
        };

        // Check RIFF header
        if (readString(0, 4) !== 'RIFF') {
          reject(new Error('Invalid WAV file: missing RIFF header'));
          return;
        }

        if (readString(8, 4) !== 'WAVE') {
          reject(new Error('Invalid WAV file: missing WAVE header'));
          return;
        }

        // Find fmt chunk
        let offset = 12;
        let sampleRate = 44100; // default
        let numChannels = 1;
        let bitsPerSample = 16;
        let dataOffset = 0;
        let dataSize = 0;

        while (offset < arrayBuffer.byteLength) {
          const chunkId = readString(offset, 4);
          const chunkSize = view.getUint32(offset + 4, true);
          
          if (chunkId === 'fmt ') {
            view.getUint16(offset + 8, true); // audioFormat (unused but read for correctness)
            numChannels = view.getUint16(offset + 10, true);
            sampleRate = view.getUint32(offset + 12, true);
            bitsPerSample = view.getUint16(offset + 22, true);
          } else if (chunkId === 'data') {
            dataOffset = offset + 8;
            dataSize = chunkSize;
            break;
          }
          
          offset += 8 + chunkSize;
        }

        if (dataOffset === 0) {
          reject(new Error('Invalid WAV file: missing data chunk'));
          return;
        }

        // Read audio data
        const bytesPerSample = bitsPerSample / 8;
        const numSamples = dataSize / bytesPerSample / numChannels;
        const samples = new Float32Array(numSamples);

        for (let i = 0; i < numSamples; i++) {
          let sample = 0;
          if (bitsPerSample === 16) {
            sample = view.getInt16(dataOffset + i * bytesPerSample * numChannels, true) / 32768.0;
          } else if (bitsPerSample === 32) {
            sample = view.getInt32(dataOffset + i * bytesPerSample * numChannels, true) / 2147483648.0;
          } else {
            reject(new Error(`Unsupported bits per sample: ${bitsPerSample}`));
            return;
          }
          samples[i] = sample;
        }

        resolve({ samples, sampleRate });
      } catch (error) {
        reject(error);
      }
    };

    reader.onerror = () => {
      reject(new Error('Failed to read file'));
    };

    reader.readAsArrayBuffer(file);
  });
}

