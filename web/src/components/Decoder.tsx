import { useState, useRef, useCallback } from 'react';
import { wavFileToSamples } from '../utils/audioUtils';
import { decodeAudioWithViz, DecodeResult } from '../wasm';
import WaveformVisualization from './WaveformVisualization';
import FrequencyDomainVisualization from './FrequencyDomainVisualization';
import BitDiagnostics from './BitDiagnostics';

export default function Decoder() {
  const [isDecoding, setIsDecoding] = useState(false);
  const [decodedMessage, setDecodedMessage] = useState<string | null>(null);
  const [decodedBytes, setDecodedBytes] = useState<number[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [fileName, setFileName] = useState<string | null>(null);
  const [decodeResult, setDecodeResult] = useState<DecodeResult | null>(null);
  const [sampleRate, setSampleRate] = useState<number>(8000);
  const dropRef = useRef<HTMLDivElement | null>(null);

  const handleDecodeFile = useCallback(async (file: File) => {
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
  }, []);

  const handleFileSelect = (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;
    handleDecodeFile(file);
  };

  const handleDrop = (event: React.DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    const file = event.dataTransfer.files?.[0];
    if (file) {
      handleDecodeFile(file);
    }
  };

  const handleDragOver = (event: React.DragEvent<HTMLDivElement>) => {
    event.preventDefault();
  };

  return (
    <div className="decoder-section">
      <h2>Decode Message</h2>
      
      <div className="form-group">
        <label htmlFor="wav-file">Drop or select a watermarked WAV file:</label>
        <div
          className="drop-zone"
          onDrop={handleDrop}
          onDragOver={handleDragOver}
          ref={dropRef}
        >
          <input
            id="wav-file"
            type="file"
            accept=".wav"
            onChange={handleFileSelect}
            disabled={isDecoding}
          />
          <span>Drop file here, or click to browse</span>
        </div>
        {fileName && <div className="file-info">Using: {fileName}</div>}
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

          <div className="waveform-section">
            <h4>Captured 32ms Frame</h4>
            <WaveformVisualization
              originalFrame={decodeResult.visualization.first_frame}
              watermarkedFrame={decodeResult.visualization.first_frame}
              sampleRate={sampleRate}
              leftLabel="Watermarked Audio (32ms frame)"
              rightLabel="Same frame (for comparison)"
            />
          </div>
          
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
              width={1400}
              height={500}
            />
          </div>

          <div className="bit-diagnostics-section">
            <h4>Per-Bit Diagnostics (scores & votes)</h4>
            <BitDiagnostics
              bitSequence={decodeResult.visualization.bit_sequence}
              scores={decodeResult.visualization.scores}
              votes={decodeResult.visualization.votes}
              threshold={decodeResult.visualization.threshold}
              startBin={48}
            />
          </div>
        </div>
      )}
    </div>
  );
}
