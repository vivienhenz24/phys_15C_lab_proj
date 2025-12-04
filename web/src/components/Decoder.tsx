import { useState } from 'react';
import { wavFileToSamples } from '../utils/audioUtils';
import { decodeAudio } from '../wasm';

export default function Decoder() {
  const [isDecoding, setIsDecoding] = useState(false);
  const [decodedMessage, setDecodedMessage] = useState<string | null>(null);
  const [decodedBytes, setDecodedBytes] = useState<number[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [fileName, setFileName] = useState<string | null>(null);

  const handleFileSelect = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;

    setFileName(file.name);
    setDecodedMessage(null);
    setDecodedBytes(null);
    setError(null);
    setIsDecoding(true);

    try {
      // Parse WAV file
      const { samples, sampleRate } = await wavFileToSamples(file);

      // Decode the watermark
      const result = await decodeAudio(samples, sampleRate);
      setDecodedMessage(result.message);
      setDecodedBytes(result.raw_bytes);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Decoding failed');
    } finally {
      setIsDecoding(false);
    }
  };

  return (
    <div className="decoder-section">
      <h2>Decode Message</h2>
      
      <div className="form-group">
        <label htmlFor="wav-file">Upload watermarked WAV file:</label>
        <input
          id="wav-file"
          type="file"
          accept=".wav"
          onChange={handleFileSelect}
          disabled={isDecoding}
        />
        {fileName && <div className="file-info">Selected: {fileName}</div>}
      </div>

      {isDecoding && <div className="decoding-indicator">Decoding...</div>}

      {decodedMessage !== null && (
        <div className="decoded-result">
          <h3>Decoded Message:</h3>
          <div className="message-display">{decodedMessage || '(empty)'}</div>
          {decodedBytes && decodedBytes.length > 0 && (
            <div className="bytes-display">
              <strong>Raw bytes:</strong> [{decodedBytes.join(', ')}]
            </div>
          )}
        </div>
      )}

      {error && <div className="error-message">{error}</div>}
    </div>
  );
}

