import { useState } from 'react';
import { wavFileToSamples } from '../utils/audioUtils';
import { decodeAudioWithViz, DecodeResult } from '../wasm';
import WaveformVisualization from './WaveformVisualization';
import FrequencyDomainVisualization from './FrequencyDomainVisualization';

export default function Decoder() {
  const [isDecoding, setIsDecoding] = useState(false);
  const [decodedMessage, setDecodedMessage] = useState<string | null>(null);
  const [decodedBytes, setDecodedBytes] = useState<number[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [fileName, setFileName] = useState<string | null>(null);
  const [decodeResult, setDecodeResult] = useState<DecodeResult | null>(null);
  const [sampleRate, setSampleRate] = useState<number>(8000);

  const handleFileSelect = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;

    setFileName(file.name);
    setDecodedMessage(null);
    setDecodedBytes(null);
    setError(null);
    setDecodeResult(null);
    setIsDecoding(true);

    try {
      // Parse WAV file
      const { samples, sampleRate: fileSampleRate } = await wavFileToSamples(file);
      setSampleRate(fileSampleRate);

      // Decode the watermark with visualization data
      const result = await decodeAudioWithViz(samples, fileSampleRate);
      setDecodedMessage(result.message);
      setDecodedBytes(result.raw_bytes);
      setDecodeResult(result);
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

      {decodeResult && decodeResult.visualization.first_frame.length > 0 && (
        <div className="visualization-section">
          <h3>Decoding Visualization</h3>
          
          <div className="frequency-domain-section">
            <h4>Frequency Domain - Decoded Bits</h4>
            <FrequencyDomainVisualization
              audioFrame={decodeResult.visualization.first_frame}
              bitSequence={decodeResult.visualization.bit_sequence}
              scores={decodeResult.visualization.scores}
              votes={decodeResult.visualization.votes}
              threshold={decodeResult.visualization.threshold}
              sampleRate={sampleRate}
              startBin={48}
            />
          </div>
        </div>
      )}
    </div>
  );
}

