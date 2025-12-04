import { useState } from 'react';
import AudioRecorder from './AudioRecorder';
import { samplesToWav } from '../utils/audioUtils';
import { encodeAudio } from '../wasm';

export default function Encoder() {
  const [message, setMessage] = useState('');
  const [isEncoding, setIsEncoding] = useState(false);
  const [recordedSamples, setRecordedSamples] = useState<Float32Array | null>(null);
  const [recordedSampleRate, setRecordedSampleRate] = useState<number>(8000);
  const [error, setError] = useState<string | null>(null);
  
  // Fixed configuration values
  const FRAME_DURATION_MS = 32;
  const STRENGTH_PERCENT = 15;

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

    try {
      // Encode the message into the audio
      const encodedSamples = await encodeAudio(
        recordedSamples,
        recordedSampleRate,
        message,
        FRAME_DURATION_MS,
        STRENGTH_PERCENT
      );

      // Convert to WAV and download
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
    </div>
  );
}

