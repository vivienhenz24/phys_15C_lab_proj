import { useState, useRef, useEffect } from 'react';
import { audioBufferToSamples } from '../utils/audioUtils';

interface AudioRecorderProps {
  onRecordingComplete: (samples: Float32Array, sampleRate: number) => void;
}

export default function AudioRecorder({ onRecordingComplete }: AudioRecorderProps) {
  const [isRecording, setIsRecording] = useState(false);
  const [duration, setDuration] = useState(0);
  const [error, setError] = useState<string | null>(null);
  
  const mediaRecorderRef = useRef<MediaRecorder | null>(null);
  const audioChunksRef = useRef<Blob[]>([]);
  const streamRef = useRef<MediaStream | null>(null);
  const durationIntervalRef = useRef<number | null>(null);
  const startTimeRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      // Cleanup on unmount
      if (streamRef.current) {
        streamRef.current.getTracks().forEach(track => track.stop());
      }
      if (durationIntervalRef.current) {
        clearInterval(durationIntervalRef.current);
      }
    };
  }, []);

  const startRecording = async () => {
    try {
      setError(null);
      const stream = await navigator.mediaDevices.getUserMedia({ 
        audio: {
          sampleRate: 8000, // Match encoder default
          channelCount: 1,
          echoCancellation: false,
          noiseSuppression: false,
        }
      });
      streamRef.current = stream;

      const mediaRecorder = new MediaRecorder(stream, {
        mimeType: 'audio/webm;codecs=opus'
      });
      mediaRecorderRef.current = mediaRecorder;
      audioChunksRef.current = [];

      mediaRecorder.ondataavailable = (event) => {
        if (event.data.size > 0) {
          audioChunksRef.current.push(event.data);
        }
      };

      mediaRecorder.onstop = async () => {
        try {
          // Combine all chunks into a single blob
          const audioBlob = new Blob(audioChunksRef.current, { type: 'audio/webm' });
          const arrayBuffer = await audioBlob.arrayBuffer();
          const audioContext = new AudioContext();
          const audioBuffer = await audioContext.decodeAudioData(arrayBuffer);
          const samples = audioBufferToSamples(audioBuffer);
          
          // Resample to 8kHz if needed (encoder expects 8kHz)
          let finalSamples = samples;
          let finalSampleRate = audioBuffer.sampleRate;
          
          if (audioBuffer.sampleRate !== 8000) {
            // Simple resampling: linear interpolation
            const ratio = 8000 / audioBuffer.sampleRate;
            const newLength = Math.floor(samples.length * ratio);
            finalSamples = new Float32Array(newLength);
            
            for (let i = 0; i < newLength; i++) {
              const srcIndex = i / ratio;
              const srcIndexFloor = Math.floor(srcIndex);
              const srcIndexCeil = Math.min(srcIndexFloor + 1, samples.length - 1);
              const frac = srcIndex - srcIndexFloor;
              finalSamples[i] = samples[srcIndexFloor] * (1 - frac) + samples[srcIndexCeil] * frac;
            }
            finalSampleRate = 8000;
          }
          
          onRecordingComplete(finalSamples, finalSampleRate);
          audioContext.close();
        } catch (err) {
          setError(err instanceof Error ? err.message : 'Failed to process recording');
        }
      };

      mediaRecorder.start();
      setIsRecording(true);
      startTimeRef.current = Date.now();
      
      // Update duration every 100ms
      durationIntervalRef.current = window.setInterval(() => {
        if (startTimeRef.current) {
          setDuration(Math.floor((Date.now() - startTimeRef.current) / 1000));
        }
      }, 100);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to start recording');
      setIsRecording(false);
    }
  };

  const stopRecording = () => {
    if (mediaRecorderRef.current && isRecording) {
      mediaRecorderRef.current.stop();
      setIsRecording(false);
      
      if (durationIntervalRef.current) {
        clearInterval(durationIntervalRef.current);
        durationIntervalRef.current = null;
      }
      
      if (streamRef.current) {
        streamRef.current.getTracks().forEach(track => track.stop());
        streamRef.current = null;
      }
    }
  };

  const formatDuration = (seconds: number): string => {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins}:${secs.toString().padStart(2, '0')}`;
  };

  return (
    <div className="audio-recorder">
      <div className="recorder-controls">
        {!isRecording ? (
          <button onClick={startRecording} className="record-button">
            Start Recording
          </button>
        ) : (
          <button onClick={stopRecording} className="stop-button">
            Stop Recording
          </button>
        )}
        {isRecording && (
          <div className="recording-indicator">
            <span className="recording-dot"></span>
            Recording: {formatDuration(duration)}
          </div>
        )}
      </div>
      {error && <div className="error-message">{error}</div>}
    </div>
  );
}

