import { useEffect, useRef } from 'react';

interface BitDiagnosticsProps {
  bitSequence: number[];
  scores?: number[];
  votes?: number[];
  threshold?: number;
  startBin?: number;
}

export default function BitDiagnostics({
  bitSequence,
  scores,
  votes,
  threshold,
  startBin = 10,
}: BitDiagnosticsProps) {
  const scoreCanvasRef = useRef<HTMLCanvasElement>(null);
  const voteCanvasRef = useRef<HTMLCanvasElement>(null);

  // Helper to draw bar charts
  const drawBars = (
    canvas: HTMLCanvasElement,
    values: number[],
    opts: {
      title: string;
      line?: number;
      lineLabel?: string;
      barLabels?: number[];
      colorMap?: (v: number, idx: number) => string;
      suffix?: string;
    }
  ) => {
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const width = canvas.width;
    const height = canvas.height;
    const padding = { top: 40, right: 20, bottom: 40, left: 60 };
    const drawW = width - padding.left - padding.right;
    const drawH = height - padding.top - padding.bottom;

    ctx.clearRect(0, 0, width, height);
    ctx.fillStyle = '#f8f9fa';
    ctx.fillRect(0, 0, width, height);

    const maxVal = Math.max(...values.map((v) => Math.abs(v)), 1e-6);
    const barW = drawW / values.length;

    // Grid
    ctx.strokeStyle = '#e0e0e0';
    ctx.lineWidth = 1;
    const gridLines = 4;
    for (let i = 0; i <= gridLines; i++) {
      const y = padding.top + (drawH * i) / gridLines;
      ctx.beginPath();
      ctx.moveTo(padding.left, y);
      ctx.lineTo(width - padding.right, y);
      ctx.stroke();
    }

    // Bars
    values.forEach((v, idx) => {
      const barH = (Math.abs(v) / maxVal) * drawH;
      const x = padding.left + idx * barW;
      const y = padding.top + drawH - barH;

      const color =
        opts.colorMap?.(v, idx) ??
        (v >= 0 ? '#FFD700' : '#4169E1');

      ctx.fillStyle = color;
      ctx.strokeStyle = '#555';
      ctx.lineWidth = 1;
      ctx.fillRect(x, y, barW * 0.9, barH);
      ctx.strokeRect(x, y, barW * 0.9, barH);

      if (barH > 20 && opts.barLabels) {
        ctx.fillStyle = '#fff';
        ctx.font = 'bold 10px "Helvetica Neue", Helvetica, Arial, sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText(
          opts.barLabels[idx].toString(),
          x + (barW * 0.9) / 2,
          y - 6
        );
      }
    });

    // Line (threshold or vote pivot)
    if (opts.line !== undefined && maxVal > 0) {
      const norm = Math.min(Math.max(opts.line / maxVal, -1), 1);
      const y = padding.top + drawH - norm * drawH;
      ctx.strokeStyle = '#FF6B6B';
      ctx.setLineDash([5, 5]);
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.moveTo(padding.left, y);
      ctx.lineTo(width - padding.right, y);
      ctx.stroke();
      ctx.setLineDash([]);
      if (opts.lineLabel) {
        ctx.fillStyle = '#FF6B6B';
        ctx.font = '12px "Helvetica Neue", Helvetica, Arial, sans-serif';
        ctx.textAlign = 'right';
        ctx.fillText(opts.lineLabel, width - padding.right - 8, y - 6);
      }
    }

    // Axes
    ctx.strokeStyle = '#333';
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.moveTo(padding.left, padding.top);
    ctx.lineTo(padding.left, padding.top + drawH);
    ctx.lineTo(width - padding.right, padding.top + drawH);
    ctx.stroke();

    // Labels
    ctx.fillStyle = '#fff';
    ctx.font = 'bold 14px "Helvetica Neue", Helvetica, Arial, sans-serif';
    ctx.textAlign = 'center';
    ctx.fillText(opts.title, width / 2, 22);

    ctx.font = '12px "Helvetica Neue", Helvetica, Arial, sans-serif';
    ctx.fillText('Watermark Bin', width / 2, height - 10);
  };

  useEffect(() => {
    if (!bitSequence.length) return;

    const len = bitSequence.length;
    const sliceScores = scores ? scores.slice(0, len) : Array(len).fill(0);
    const sliceVotes = votes ? votes.slice(0, len) : Array(len).fill(0);

    if (scoreCanvasRef.current) {
      drawBars(scoreCanvasRef.current, sliceScores, {
        title: `Spectral scores vs threshold (bins ${startBin}–${startBin + len - 1})`,
        line: threshold,
        lineLabel: 'Threshold',
        barLabels: bitSequence,
        colorMap: (_v, idx) => (bitSequence[idx] === 1 ? '#FFD700' : '#4169E1'),
      });
    }

    if (voteCanvasRef.current) {
      drawBars(voteCanvasRef.current, sliceVotes, {
        title: 'Vote ratios (1.0 = unanimous "1")',
        line: 0.5,
        lineLabel: '0.5',
        barLabels: bitSequence,
        colorMap: (v, idx) =>
          v >= 0.5 ? '#FFD700' : '#4169E1',
        suffix: '',
      });
    }
  }, [bitSequence, scores, votes, threshold, startBin]);

  return (
    <div className="bit-diagnostics">
      <canvas
        ref={scoreCanvasRef}
        width={1400}
        height={280}
        style={{ width: '100%', border: '1px solid #ddd', borderRadius: '6px' }}
      />
      <canvas
        ref={voteCanvasRef}
        width={1400}
        height={220}
        style={{ width: '100%', border: '1px solid #ddd', borderRadius: '6px' }}
      />
    </div>
  );
}
