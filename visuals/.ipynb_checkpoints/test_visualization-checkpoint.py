#!/usr/bin/env python3
"""
Test script to verify audio loading and visualization works correctly
"""

import numpy as np
import matplotlib
matplotlib.use('Agg')  # Use non-interactive backend
import matplotlib.pyplot as plt
from scipy.io import wavfile

print("=" * 70)
print("Testing Audio Visualization")
print("=" * 70)

# File paths
original_path = '../input_data/OSR_us_000_0057_8k.wav'
watermarked_path = '../output_data/OSR_us_000_0057_8k_watermarked.wav'

# Load audio files
print("\n1. Loading audio files...")
sample_rate_orig, audio_orig = wavfile.read(original_path)
sample_rate_wm, audio_wm = wavfile.read(watermarked_path)

# Normalize to [-1, 1]
audio_orig_norm = audio_orig.astype(np.float32) / 32768.0
audio_wm_norm = audio_wm.astype(np.float32) / 32768.0

print(f"   ✓ Original audio: {len(audio_orig)} samples at {sample_rate_orig} Hz")
print(f"   ✓ Watermarked audio: {len(audio_wm)} samples at {sample_rate_wm} Hz")
print(f"   ✓ Duration: {len(audio_orig) / sample_rate_orig:.2f} seconds")

# Create time axis
print("\n2. Creating time axis...")
time_orig = np.arange(len(audio_orig_norm)) / sample_rate_orig
time_wm = np.arange(len(audio_wm_norm)) / sample_rate_wm

# Plot first 0.1 seconds
duration_to_plot = 0.1
samples_to_plot = int(duration_to_plot * sample_rate_orig)

print(f"   ✓ Time axis created: {len(time_orig)} points")
print(f"   ✓ Plotting {samples_to_plot} samples (first {duration_to_plot}s)")

# Check data ranges
print("\n3. Checking data ranges...")
print(f"   Original audio range: [{audio_orig_norm[:samples_to_plot].min():.4f}, {audio_orig_norm[:samples_to_plot].max():.4f}]")
print(f"   Watermarked audio range: [{audio_wm_norm[:samples_to_plot].min():.4f}, {audio_wm_norm[:samples_to_plot].max():.4f}]")
print(f"   Time range: [{time_orig[:samples_to_plot].min():.4f}, {time_orig[:samples_to_plot].max():.4f}] seconds")

# Create plot
print("\n4. Creating visualization...")
fig, (ax1, ax2, ax3) = plt.subplots(3, 1, figsize=(14, 10))

# Original waveform
ax1.plot(time_orig[:samples_to_plot], audio_orig_norm[:samples_to_plot], 
         linewidth=0.5, color='blue', alpha=0.7)
ax1.set_title('Original Audio Waveform (First 0.1s)', fontsize=14, fontweight='bold')
ax1.set_xlabel('Time (seconds)')
ax1.set_ylabel('Amplitude')
ax1.grid(True, alpha=0.3)
ax1.set_ylim(-1.0, 1.0)

# Watermarked waveform
ax2.plot(time_wm[:samples_to_plot], audio_wm_norm[:samples_to_plot], 
         linewidth=0.5, color='red', alpha=0.7)
ax2.set_title('Watermarked Audio Waveform (First 0.1s)', fontsize=14, fontweight='bold')
ax2.set_xlabel('Time (seconds)')
ax2.set_ylabel('Amplitude')
ax2.grid(True, alpha=0.3)
ax2.set_ylim(-1.0, 1.0)

# Difference (amplified)
difference = audio_wm_norm[:samples_to_plot] - audio_orig_norm[:samples_to_plot]
ax3.plot(time_orig[:samples_to_plot], difference * 10, 
         linewidth=0.5, color='green', alpha=0.7)
ax3.set_title('Difference (Watermark Signal, 10x Amplified)', fontsize=14, fontweight='bold')
ax3.set_xlabel('Time (seconds)')
ax3.set_ylabel('Amplitude Difference')
ax3.grid(True, alpha=0.3)

plt.tight_layout()
output_file = 'test_waveform.png'
plt.savefig(output_file, dpi=150)
print(f"   ✓ Plot saved to {output_file}")

print(f"\n5. Statistics:")
print(f"   Max difference: {np.max(np.abs(difference)):.6f}")
print(f"   RMS difference: {np.sqrt(np.mean(difference**2)):.6f}")
print(f"   Mean original amplitude: {np.mean(np.abs(audio_orig_norm[:samples_to_plot])):.6f}")
print(f"   Mean watermarked amplitude: {np.mean(np.abs(audio_wm_norm[:samples_to_plot])):.6f}")

print("\n" + "=" * 70)
print("✓ Test completed successfully!")
print("=" * 70)

