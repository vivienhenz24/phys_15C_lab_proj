import { useEffect, useRef } from 'react';

interface WaveformVisualizationProps {
  originalFrame: Float32Array | number[];
  watermarkedFrame: Float32Array | number[];
  sampleRate: number;
}

export default function WaveformVisualization({
  originalFrame,
  watermarkedFrame,
  sampleRate,
}: WaveformVisualizationProps) {
  const canvasRef1 = useRef<HTMLCanvasElement>(null);
  const canvasRef2 = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const peak =
      Math.max(
        ...[...originalFrame, ...watermarkedFrame].map((v) => Math.abs(v)),
        1e-6
      ) || 1e-6; // shared scaling so left/right are comparable

    const drawWaveform = (
      canvas: HTMLCanvasElement,
      samples: Float32Array | number[],
      color: string,
      label: string
    ) => {
      const ctx = canvas.getContext('2d');
      if (!ctx) return;

      const width = canvas.width;
      const height = canvas.height;
      const padding = 20;

      // Clear canvas
      ctx.clearRect(0, 0, width, height);

      // Set background
      ctx.fillStyle = '#f8f9fa';
      ctx.fillRect(0, 0, width, height);

      // Draw grid
      ctx.strokeStyle = '#e0e0e0';
      ctx.lineWidth = 1;
      const gridLines = 5;
      for (let i = 0; i <= gridLines; i++) {
        const y = padding + (height - 2 * padding) * (i / gridLines);
        ctx.beginPath();
        ctx.moveTo(padding, y);
        ctx.lineTo(width - padding, y);
        ctx.stroke();
      }

      // Draw center line
      ctx.strokeStyle = '#999';
      ctx.lineWidth = 1;
      const centerY = height / 2;
      ctx.beginPath();
      ctx.moveTo(padding, centerY);
      ctx.lineTo(width - padding, centerY);
      ctx.stroke();

      if (samples.length === 0) return;

      // Draw waveform
      ctx.strokeStyle = color;
      ctx.lineWidth = 2;
      ctx.beginPath();

      const drawWidth = width - 2 * padding;
      const drawHeight = height - 2 * padding;
      const centerYPos = padding + drawHeight / 2;

      for (let i = 0; i < samples.length; i++) {
        const x = padding + (i / (samples.length - 1)) * drawWidth;
        const sample = samples[i];
        const y = centerYPos - (sample / peak) * (drawHeight / 2);
        
        if (i === 0) {
          ctx.moveTo(x, y);
        } else {
          ctx.lineTo(x, y);
        }
      }

      ctx.stroke();

      // Draw title
      ctx.fillStyle = '#fff';
      ctx.font = 'bold 14px sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText(label, width / 2, 15);
    };

    if (canvasRef1.current) {
      drawWaveform(
        canvasRef1.current,
        originalFrame,
        '#4169E1',
        'Original Audio (32ms frame)'
      );
    }

    if (canvasRef2.current) {
      drawWaveform(
        canvasRef2.current,
        watermarkedFrame,
        '#FF6B6B',
        'Watermarked Audio (32ms frame)'
      );
    }
  }, [originalFrame, watermarkedFrame, sampleRate]);

  return (
    <div className="waveform-visualization">
      <div className="waveform-comparison-label">
        <span>Original (left)</span>
        <span className="waveform-arrow">⇄</span>
        <span>Watermarked (right)</span>
      </div>
      <div className="waveform-container">
        <div className="waveform-item">
          <canvas
            ref={canvasRef1}
            width={600}
            height={200}
            style={{ border: '1px solid #ddd', borderRadius: '4px' }}
          />
          <div className="waveform-label">Original Audio (32ms frame)</div>
        </div>
        <div className="waveform-item">
          <canvas
            ref={canvasRef2}
            width={600}
            height={200}
            style={{ border: '1px solid #ddd', borderRadius: '4px' }}
          />
          <div className="waveform-label">Watermarked Audio (32ms frame)</div>
        </div>
      </div>
    </div>
  );
}
