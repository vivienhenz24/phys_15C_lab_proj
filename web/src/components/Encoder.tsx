import { useState, useEffect } from 'react';
import AudioRecorder from './AudioRecorder';
import { samplesToWav } from '../utils/audioUtils';
import { encodeAudioWithViz, EncodeResult } from '../wasm';
import WaveformVisualization from './WaveformVisualization';
import BitSequenceVisualization from './BitSequenceVisualization';
import FrequencyDomainVisualization from './FrequencyDomainVisualization';

export default function Encoder() {
  const [message, setMessage] = useState('');
  const [isEncoding, setIsEncoding] = useState(false);
  const [recordedSamples, setRecordedSamples] = useState<Float32Array | null>(null);
  const [recordedSampleRate, setRecordedSampleRate] = useState<number>(8000);
  const [error, setError] = useState<string | null>(null);
  const [encodeResult, setEncodeResult] = useState<EncodeResult | null>(null);
  const [playbackUrl, setPlaybackUrl] = useState<string | null>(null);
  const [originalUrl, setOriginalUrl] = useState<string | null>(null);
  
  // Fixed configuration values
  const FRAME_DURATION_MS = 32;
  const STRENGTH_PERCENT = 15;
  const MAX_MESSAGE_LENGTH = 11;

  useEffect(() => {
    return () => {
      if (playbackUrl) {
        URL.revokeObjectURL(playbackUrl);
      }
      if (originalUrl) {
        URL.revokeObjectURL(originalUrl);
      }
    };
  }, [playbackUrl, originalUrl]);

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

    if (message.length > MAX_MESSAGE_LENGTH) {
      setError(`Message is too long. Maximum ${MAX_MESSAGE_LENGTH} characters allowed.`);
      return;
    }

    setIsEncoding(true);
    setError(null);
    setEncodeResult(null);
    if (playbackUrl) {
      URL.revokeObjectURL(playbackUrl);
      setPlaybackUrl(null);
    }
    if (originalUrl) {
      URL.revokeObjectURL(originalUrl);
      setOriginalUrl(null);
    }

    try {
      // Prepare original preview/download
      const originalBlob = samplesToWav(recordedSamples, recordedSampleRate);
      const originalObjectUrl = URL.createObjectURL(originalBlob);
      setOriginalUrl(originalObjectUrl);

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
      setPlaybackUrl(url);
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
        <label htmlFor="message">
          Message to encode:
          <span className="char-count">
            {message.length}/{MAX_MESSAGE_LENGTH}
          </span>
        </label>
        <textarea
          id="message"
          value={message}
          onChange={(e) => {
            if (e.target.value.length <= MAX_MESSAGE_LENGTH) {
              setMessage(e.target.value);
            }
          }}
          placeholder="Enter your message here..."
          rows={3}
          maxLength={MAX_MESSAGE_LENGTH}
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
        {isEncoding ? 'Encoding...' : 'Encode & Preview'}
      </button>

      {(playbackUrl || originalUrl) && (
        <div className="player-card">
          <div className="player-text">Listen and compare:</div>
          {originalUrl && (
            <div className="player-actions column">
              <span className="player-label">Original</span>
              <audio controls src={originalUrl} />
              <a className="download-link" href={originalUrl} download="original_audio.wav">
                Download original
              </a>
            </div>
          )}
          {playbackUrl && (
            <div className="player-actions column">
              <span className="player-label">Watermarked</span>
              <audio controls src={playbackUrl} />
              <a className="download-link" href={playbackUrl} download="watermarked_audio.wav">
                Download watermarked
              </a>
            </div>
          )}
        </div>
      )}

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

            <div className="frequency-domain-section">
              <h4>Frequency Domain (Watermarked 32ms frame)</h4>
              <FrequencyDomainVisualization
                audioFrame={encodeResult.visualization.watermarked_frame}
                bitSequence={encodeResult.visualization.bit_sequence}
                sampleRate={recordedSampleRate}
                startBin={48}
                width={1200}
                height={360}
              />
            </div>
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
