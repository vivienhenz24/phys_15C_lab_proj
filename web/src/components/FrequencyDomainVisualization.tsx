import { useEffect, useRef, useMemo } from 'react';

interface FrequencyDomainVisualizationProps {
  audioFrame: Float32Array | number[];
  bitSequence: number[];
  scores?: number[];
  votes?: number[];
  threshold?: number;
  sampleRate: number;
  startBin?: number; // First watermark bin (default 10)
  height?: number;
  width?: number;
}

export default function FrequencyDomainVisualization({
  audioFrame,
  bitSequence,
  scores,
  votes,
  threshold,
  sampleRate,
  startBin = 10,
  height = 480,
  width = 1400,
}: FrequencyDomainVisualizationProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  // Compute FFT using DFT (Discrete Fourier Transform)
  const fftData = useMemo(() => {
    if (audioFrame.length === 0) return null;

    // Convert to Float32Array if needed
    const samples = audioFrame instanceof Float32Array 
      ? audioFrame 
      : new Float32Array(audioFrame);

    // Use next power of 2 for FFT size
    const fftSize = Math.pow(2, Math.ceil(Math.log2(samples.length)));
    
    // Zero-pad samples to fftSize
    const paddedSamples = new Float32Array(fftSize);
    for (let i = 0; i < samples.length; i++) {
      paddedSamples[i] = samples[i];
    }
    
    // Perform DFT (Discrete Fourier Transform)
    const magnitudes: number[] = [];
    const N = fftSize;
    
    for (let k = 0; k < N / 2; k++) {
      let real = 0;
      let imag = 0;
      
      for (let n = 0; n < N; n++) {
        const angle = (2 * Math.PI * k * n) / N;
        real += paddedSamples[n] * Math.cos(angle);
        imag -= paddedSamples[n] * Math.sin(angle);
      }
      
      const magnitude = Math.sqrt(real * real + imag * imag);
      magnitudes.push(magnitude);
    }
    
    return {
      magnitudes,
      frequencies: magnitudes.map((_, k) => (k * sampleRate) / fftSize),
    };
  }, [audioFrame, sampleRate]);

  useEffect(() => {
    if (!canvasRef.current || !fftData) return;

    const canvas = canvasRef.current;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const width = canvas.width;
    const height = canvas.height;
    const padding = { top: 40, right: 20, bottom: 60, left: 60 };

    // Clear canvas
    ctx.clearRect(0, 0, width, height);

    // Set background
    ctx.fillStyle = '#f8f9fa';
    ctx.fillRect(0, 0, width, height);

    const drawWidth = width - padding.left - padding.right;
    const drawHeight = height - padding.top - padding.bottom;

    // Find max magnitude for scaling
    const maxMagnitude = Math.max(...fftData.magnitudes);
    if (maxMagnitude === 0) return;

    // Draw grid
    ctx.strokeStyle = '#e0e0e0';
    ctx.lineWidth = 1;
    const gridLines = 5;
    for (let i = 0; i <= gridLines; i++) {
      const y = padding.top + (drawHeight * i) / gridLines;
      ctx.beginPath();
      ctx.moveTo(padding.left, y);
      ctx.lineTo(width - padding.right, y);
      ctx.stroke();
    }

    // Prepare geometry
    const numBins = fftData.magnitudes.length;
    const barWidth = drawWidth / numBins;
    const watermarkEndBin = Math.min(startBin + bitSequence.length, numBins);

    // Draw watermark region background
    if (watermarkEndBin > startBin) {
      const startX = padding.left + startBin * barWidth;
      const endX = padding.left + watermarkEndBin * barWidth;
      ctx.fillStyle = 'rgba(255, 215, 0, 0.08)';
      ctx.fillRect(startX, padding.top, endX - startX, drawHeight);
    }

    // Draw frequency bars

    // Draw all frequency bars
    for (let i = 0; i < numBins; i++) {
      const magnitude = fftData.magnitudes[i];
      const barHeight = (magnitude / maxMagnitude) * drawHeight;
      const x = padding.left + i * barWidth;
      const y = padding.top + drawHeight - barHeight;

      // Check if this bin is part of the watermark
      const isWatermarkBin = i >= startBin && i < watermarkEndBin;
      const bitIndex = isWatermarkBin ? i - startBin : -1;
      const decodedBit = isWatermarkBin && bitIndex < bitSequence.length 
        ? bitSequence[bitIndex] 
        : null;

      // Color based on watermark detection
      if (isWatermarkBin && decodedBit !== null) {
        // Highlight watermark bins
        ctx.fillStyle = decodedBit === 1 ? '#FFD700' : '#4169E1'; // Gold for 1, Blue for 0
        ctx.strokeStyle = '#000';
        ctx.lineWidth = 2;
      } else {
        // Regular bins
        ctx.fillStyle = '#999';
        ctx.strokeStyle = '#666';
        ctx.lineWidth = 1;
      }

      ctx.fillRect(x, y, barWidth - 1, barHeight);
      ctx.strokeRect(x, y, barWidth - 1, barHeight);

      // Draw bit value label on watermark bins
      if (isWatermarkBin && decodedBit !== null && barHeight > 10) {
        ctx.fillStyle = '#fff';
        ctx.font = 'bold 10px "Helvetica Neue", Helvetica, Arial, sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText(
          decodedBit.toString(),
          x + barWidth / 2,
          y - 5
        );
      }
    }

    // Draw watermark region highlight
    if (watermarkEndBin > startBin) {
      const watermarkStartX = padding.left + startBin * barWidth;
      const watermarkEndX = padding.left + watermarkEndBin * barWidth;
      
      ctx.strokeStyle = '#FF6B6B';
      ctx.lineWidth = 2;
      ctx.setLineDash([5, 5]);
      ctx.beginPath();
      ctx.moveTo(watermarkStartX, padding.top);
      ctx.lineTo(watermarkStartX, padding.top + drawHeight);
      ctx.moveTo(watermarkEndX, padding.top);
      ctx.lineTo(watermarkEndX, padding.top + drawHeight);
      ctx.stroke();
      ctx.setLineDash([]);
    }

    // Draw axes
    ctx.strokeStyle = '#333';
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.moveTo(padding.left, padding.top);
    ctx.lineTo(padding.left, padding.top + drawHeight);
    ctx.lineTo(width - padding.right, padding.top + drawHeight);
    ctx.stroke();

    // Draw labels
    ctx.fillStyle = '#fff';
    ctx.font = '12px "Helvetica Neue", Helvetica, Arial, sans-serif';
    ctx.textAlign = 'center';
    
    // X-axis label
    ctx.fillText('Frequency Bin', width / 2, height - 10);
    
    // Y-axis label
    ctx.save();
    ctx.translate(15, height / 2);
    ctx.rotate(-Math.PI / 2);
    ctx.fillText('Magnitude', 0, 0);
    ctx.restore();

    // Draw frequency bin numbers for watermark region
    ctx.font = '10px "Helvetica Neue", Helvetica, Arial, sans-serif';
    ctx.fillStyle = '#fff';
    for (let i = startBin; i < watermarkEndBin; i += Math.max(1, Math.floor((watermarkEndBin - startBin) / 10))) {
      const x = padding.left + i * barWidth + barWidth / 2;
      ctx.fillText(i.toString(), x, height - padding.bottom + 15);
    }

    // Draw title
    ctx.fillStyle = '#fff';
    ctx.font = 'bold 16px "Helvetica Neue", Helvetica, Arial, sans-serif';
    ctx.textAlign = 'center';
    ctx.fillText('Frequency Domain - Watermark Detection', width / 2, 25);
  }, [fftData, bitSequence, startBin, scores, votes, threshold]);

  return (
    <div className="frequency-domain-visualization">
      <canvas
        ref={canvasRef}
        width={width}
        height={height}
        style={{ border: '1px solid #ddd', borderRadius: '4px', maxWidth: '100%' }}
      />
      <div className="frequency-legend">
        <div className="legend-item">
          <div className="legend-color" style={{ backgroundColor: '#999' }}></div>
          <span>Regular Frequency Bins</span>
        </div>
        <div className="legend-item">
          <div className="legend-color" style={{ backgroundColor: '#FFD700' }}></div>
          <span>Watermark Bit 1</span>
        </div>
        <div className="legend-item">
          <div className="legend-color" style={{ backgroundColor: '#4169E1' }}></div>
          <span>Watermark Bit 0</span>
        </div>
        <div className="legend-item">
          <div className="legend-dash" style={{ borderTop: '2px dashed #FF6B6B' }}></div>
          <span>Watermark Region</span>
        </div>
      </div>
    </div>
  );
}
