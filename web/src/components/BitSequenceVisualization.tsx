import { useMemo } from 'react';

interface BitSequenceVisualizationProps {
  bitSequence: number[];
  scores?: number[];
  votes?: number[];
  threshold?: number;
  avgHigh?: number;
  avgLow?: number;
  inverted?: boolean;
}

export default function BitSequenceVisualization({
  bitSequence,
  scores,
  votes,
  threshold,
  avgHigh,
  avgLow,
  inverted,
}: BitSequenceVisualizationProps) {
  // Helper function to convert bits to byte value
  const bitsToByte = (bits: number[]): number => {
    if (bits.length !== 8) return 0;
    let byte = 0;
    for (let i = 0; i < 8; i++) {
      byte = (byte << 1) | (bits[i] & 1);
    }
    return byte;
  };

  // Helper function to get character from byte
  const byteToChar = (byte: number): string => {
    try {
      const char = String.fromCharCode(byte);
      // Show printable characters, otherwise show hex
      if (char >= ' ' && char <= '~') {
        return char;
      }
      return `\\x${byte.toString(16).padStart(2, '0')}`;
    } catch {
      return `\\x${byte.toString(16).padStart(2, '0')}`;
    }
  };

  // Helper function to decode 16-bit length header
  const bitsToLength = (bits: number[]): number => {
    if (bits.length !== 16) return 0;
    let length = 0;
    for (let i = 0; i < 16; i++) {
      length = (length << 1) | (bits[i] & 1);
    }
    return length;
  };

  const bitGroups = useMemo(() => {
    // Group bits: pilot (8), length header (16), then message bytes (8 each)
    const groups: Array<{
      label: string;
      bits: number[];
      startIdx: number;
      color: string;
      character?: string;
    }> = [];

    if (bitSequence.length === 0) return groups;

    // Pilot pattern (first 8 bits)
    if (bitSequence.length >= 8) {
      groups.push({
        label: 'Pilot (8 bits)',
        bits: bitSequence.slice(0, 8),
        startIdx: 0,
        color: '#FFD700',
      });
    }

    // Length header (next 16 bits)
    if (bitSequence.length >= 24) {
      const lengthBits = bitSequence.slice(8, 24);
      const lengthValue = bitsToLength(lengthBits);
      groups.push({
        label: `Length Header (16 bits) → ${lengthValue} bytes`,
        bits: lengthBits,
        startIdx: 8,
        color: '#FF6B6B',
      });
    }

    // Message bits (remaining, grouped by bytes)
    const messageStart = 24;
    if (bitSequence.length > messageStart) {
      const messageBits = bitSequence.slice(messageStart);
      const numBytes = Math.floor(messageBits.length / 8);
      
      for (let i = 0; i < numBytes; i++) {
        const byteStart = messageStart + i * 8;
        const byteBits = bitSequence.slice(byteStart, byteStart + 8);
        const byteValue = bitsToByte(byteBits);
        const character = byteToChar(byteValue);
        
        groups.push({
          label: `Message Byte ${i + 1} (8 bits) → '${character}'`,
          bits: byteBits,
          startIdx: byteStart,
          color: '#4169E1',
          character: character,
        });
      }
    }

    return groups;
  }, [bitSequence]);

  const getBitColor = (bit: number, idx: number): string => {
    if (bit === 1) return '#FFD700'; // Gold for 1
    return '#4169E1'; // Royal blue for 0
  };

  const getBitConfidence = (idx: number): number | null => {
    if (scores && votes && threshold !== undefined) {
      const score = scores[idx];
      const vote = votes[idx];
      
      if (score !== undefined && vote !== undefined) {
        // Confidence based on how far from threshold and vote ratio
        const distanceFromThreshold = Math.abs(score - (threshold || 0));
        const voteConfidence = Math.abs(vote - 0.5) * 2; // 0 to 1
        return Math.min(1.0, (distanceFromThreshold / Math.max(0.1, Math.abs((avgHigh || 0) - (avgLow || 0)))) * 0.5 + voteConfidence * 0.5);
      }
    }
    return null;
  };

  return (
    <div className="bit-sequence-visualization">
      <h3>Bit Sequence Visualization</h3>
      <div className="bit-sequence-container">
        {bitGroups.map((group, groupIdx) => (
          <div key={groupIdx} className="bit-group">
            <div className="bit-group-header" style={{ backgroundColor: group.color + '20' }}>
              <span className="bit-group-label">{group.label}</span>
            </div>
            <div className="bit-group-bits">
              {group.bits.map((bit, bitIdx) => {
                const globalIdx = group.startIdx + bitIdx;
                const confidence = getBitConfidence(globalIdx);
                const bitColor = getBitColor(bit, globalIdx);
                
                return (
                  <div
                    key={bitIdx}
                    className="bit-box"
                    style={{
                      backgroundColor: bitColor,
                      opacity: confidence !== null ? 0.6 + confidence * 0.4 : 0.8,
                      border: confidence !== null && confidence > 0.7 ? '2px solid #00ff00' : '1px solid #333',
                    }}
                    title={
                      confidence !== null
                        ? `Bit ${globalIdx}: ${bit} (confidence: ${(confidence * 100).toFixed(1)}%)`
                        : `Bit ${globalIdx}: ${bit}`
                    }
                  >
                    {bit}
                  </div>
                );
              })}
            </div>
          </div>
        ))}
      </div>
      
      {scores && threshold !== undefined && (
        <div className="bit-sequence-info">
          <div className="info-item">
            <span className="info-label">Threshold:</span>
            <span className="info-value">{threshold.toFixed(4)}</span>
          </div>
          {avgHigh !== undefined && avgLow !== undefined && (
            <>
              <div className="info-item">
                <span className="info-label">Avg High:</span>
                <span className="info-value">{avgHigh.toFixed(4)}</span>
              </div>
              <div className="info-item">
                <span className="info-label">Avg Low:</span>
                <span className="info-value">{avgLow.toFixed(4)}</span>
              </div>
            </>
          )}
          {inverted !== undefined && (
            <div className="info-item">
              <span className="info-label">Inverted:</span>
              <span className="info-value">{inverted ? 'Yes' : 'No'}</span>
            </div>
          )}
        </div>
      )}
      
      <div className="bit-legend">
        <div className="legend-item">
          <div className="legend-color" style={{ backgroundColor: '#FFD700' }}></div>
          <span>Bit 1</span>
        </div>
        <div className="legend-item">
          <div className="legend-color" style={{ backgroundColor: '#4169E1' }}></div>
          <span>Bit 0</span>
        </div>
        {scores && (
          <div className="legend-item">
            <div className="legend-color" style={{ border: '2px solid #00ff00' }}></div>
            <span>High Confidence (&gt;70%)</span>
          </div>
        )}
      </div>
    </div>
  );
}

