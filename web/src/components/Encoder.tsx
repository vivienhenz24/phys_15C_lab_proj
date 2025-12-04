import { useState } from 'react';
import AudioRecorder from './AudioRecorder';
import { samplesToWav } from '../utils/audioUtils';
import { encodeAudioWithViz, EncodeResult } from '../wasm';
import WaveformVisualization from './WaveformVisualization';
import BitSequenceVisualization from './BitSequenceVisualization';

export default function Encoder() {
  const [message, setMessage] = useState('');
  const [isEncoding, setIsEncoding] = useState(false);
  const [recordedSamples, setRecordedSamples] = useState<Float32Array | null>(null);
  const [recordedSampleRate, setRecordedSampleRate] = useState<number>(8000);
  const [error, setError] = useState<string | null>(null);
  const [encodeResult, setEncodeResult] = useState<EncodeResult | null>(null);
  
  // Fixed configuration values
  const FRAME_DURATION_MS = 32;
  const STRENGTH_PERCENT = 50;

  const handleRecordingComplete = (samples: Float32Array, sampleRate: number) => {
    setRecordedSamples(samples);
    setRecordedSampleRate(sampleRate);
    setError(null);
  };

  const handleEncode = async () => {
    if (!recordedSamples) {
      setError('Please record audio first');
      return;
    }

    if (!message.trim()) {
      setError('Please enter a message to encode');
      return;
    }

    setIsEncoding(true);
    setError(null);
    setEncodeResult(null);

    try {
      // Encode the message into the audio with visualization data
      const result = await encodeAudioWithViz(
        recordedSamples,
        recordedSampleRate,
        message,
        FRAME_DURATION_MS,
        STRENGTH_PERCENT
      );

      setEncodeResult(result);

      // Convert to WAV and download
      const encodedSamples = new Float32Array(result.encoded_samples);
      const wavBlob = samplesToWav(encodedSamples, recordedSampleRate);
      const url = URL.createObjectURL(wavBlob);
      const a = document.createElement('a');
      a.href = url;
      a.download = 'watermarked_audio.wav';
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Encoding failed');
    } finally {
      setIsEncoding(false);
    }
  };

  return (
    <div className="encoder-section">
      <h2>Encode Message</h2>
      
      <div className="form-group">
        <label htmlFor="message">Message to encode:</label>
        <textarea
          id="message"
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          placeholder="Enter your message here..."
          rows={3}
        />
      </div>

      <div className="form-group">
        <label>Record Audio:</label>
        <AudioRecorder onRecordingComplete={handleRecordingComplete} />
        {recordedSamples && (
          <div className="recording-info">
            Recorded {recordedSamples.length} samples at {recordedSampleRate} Hz
          </div>
        )}
      </div>

      <button
        onClick={handleEncode}
        disabled={isEncoding || !recordedSamples || !message.trim()}
        className="encode-button"
      >
        {isEncoding ? 'Encoding...' : 'Encode & Download WAV'}
      </button>

      {error && <div className="error-message">{error}</div>}

      {encodeResult && (
        <div className="visualization-section">
          <h3>Encoding Visualization</h3>
          
          <div className="waveform-section">
            <h4>32ms Frame Comparison</h4>
            <WaveformVisualization
              originalFrame={encodeResult.visualization.original_frame}
              watermarkedFrame={encodeResult.visualization.watermarked_frame}
              sampleRate={recordedSampleRate}
            />
          </div>

          <div className="bit-sequence-section">
            <BitSequenceVisualization
              bitSequence={encodeResult.visualization.bit_sequence}
            />
          </div>
        </div>
      )}
    </div>
  );
}

