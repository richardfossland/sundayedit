//! Audio extraction + waveform peaks — Phase 1.2.
//!
//! The waveform is the user's main spatial reference in the editor, so it
//! must be instant. We:
//!   1. Extract the audio to a 16 kHz mono WAV via ffmpeg (also exactly
//!      what Whisper wants as input in Phase 2 — one extraction, two uses).
//!   2. Read the WAV samples (`hound`).
//!   3. Downsample to peak data at multiple zoom levels, cached to disk.
//!
//! The peak computation is a pure function tested against synthetic
//! samples — no audio file or ffmpeg required for the unit tests.

use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{AppError, AppResult};
use crate::services::video::ffmpeg_path;

/// Whisper wants 16 kHz mono; the waveform is happy with it too.
pub const TARGET_SAMPLE_RATE: u32 = 16_000;

/// One vertical slice of the waveform: the min and max sample in a bucket.
/// Rendering draws a vertical line from `min` to `max` per pixel column.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/Peak.ts")]
pub struct Peak {
    pub min: f32,
    pub max: f32,
}

/// Multi-resolution peak data. `levels[0]` is the coarsest (whole file in
/// few buckets); higher indices are finer. The editor picks the level
/// closest to the current pixel-per-second zoom.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/lib/bindings/WaveformData.ts")]
pub struct WaveformData {
    pub sample_rate: u32,
    #[ts(type = "number")]
    pub total_samples: u64,
    /// One entry per zoom level; each is a Vec<Peak>.
    pub levels: Vec<Vec<Peak>>,
}

/// Extract audio to a 16 kHz mono WAV at `out_wav` using ffmpeg.
/// Returns the command that was run on success (for logging/diagnostics).
pub fn extract_audio_wav(input: &Path, out_wav: &Path) -> AppResult<()> {
    if !input.exists() {
        return Err(AppError::VideoMissing(input.to_string_lossy().to_string()));
    }
    if let Some(parent) = out_wav.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let status = Command::new(ffmpeg_path())
        .arg("-y") // overwrite
        .arg("-i")
        .arg(input)
        .args(["-ac", "1"]) // mono
        .args(["-ar", &TARGET_SAMPLE_RATE.to_string()]) // 16 kHz
        .args(["-c:a", "pcm_s16le"]) // 16-bit PCM
        .arg("-vn") // drop video
        .arg(out_wav)
        .status()
        .map_err(|e| AppError::Internal(format!("failed to launch ffmpeg: {e}")))?;

    if !status.success() {
        return Err(AppError::Internal(format!(
            "ffmpeg audio extraction failed for '{}'",
            input.display()
        )));
    }
    Ok(())
}

/// Read a 16-bit PCM mono WAV and return normalized f32 samples in [-1, 1].
pub fn read_wav_samples(path: &Path) -> AppResult<(Vec<f32>, u32)> {
    let reader = hound::WavReader::open(path)
        .map_err(|e| AppError::Internal(format!("failed to open WAV: {e}")))?;
    let spec = reader.spec();
    let sample_rate = spec.sample_rate;

    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let max = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .into_samples::<i32>()
                .filter_map(Result::ok)
                .map(|s| s as f32 / max)
                .collect()
        }
        hound::SampleFormat::Float => reader
            .into_samples::<f32>()
            .filter_map(Result::ok)
            .collect(),
    };
    Ok((samples, sample_rate))
}

/// Downsample samples into `bucket_count` peaks. Each peak holds the min
/// and max sample within its bucket — this preserves the visual envelope
/// of the waveform even at extreme zoom-out (a transient spike still
/// shows because it becomes the bucket's max).
pub fn compute_peaks(samples: &[f32], bucket_count: usize) -> Vec<Peak> {
    if samples.is_empty() || bucket_count == 0 {
        return Vec::new();
    }
    let bucket_count = bucket_count.min(samples.len());
    let per_bucket = samples.len() as f64 / bucket_count as f64;

    let mut peaks = Vec::with_capacity(bucket_count);
    for i in 0..bucket_count {
        let start = (i as f64 * per_bucket).floor() as usize;
        let end = (((i + 1) as f64 * per_bucket).floor() as usize)
            .min(samples.len())
            .max(start + 1);
        let slice = &samples[start..end];
        let mut min = f32::MAX;
        let mut max = f32::MIN;
        for &s in slice {
            if s < min {
                min = s;
            }
            if s > max {
                max = s;
            }
        }
        peaks.push(Peak { min, max });
    }
    peaks
}

/// Build multi-resolution peak data. `base_buckets` is the coarsest
/// level; each subsequent level multiplies by `factor`. Levels stop once
/// a level would have more buckets than samples (no point oversampling).
pub fn compute_levels(
    samples: &[f32],
    sample_rate: u32,
    base_buckets: usize,
    factor: usize,
    max_levels: usize,
) -> WaveformData {
    let mut levels = Vec::new();
    let mut buckets = base_buckets.max(1);
    for _ in 0..max_levels {
        levels.push(compute_peaks(samples, buckets));
        if buckets >= samples.len() {
            break;
        }
        buckets = buckets.saturating_mul(factor.max(2));
    }
    WaveformData {
        sample_rate,
        total_samples: samples.len() as u64,
        levels,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_samples_produce_no_peaks() {
        assert!(compute_peaks(&[], 10).is_empty());
        assert!(compute_peaks(&[0.1, 0.2], 0).is_empty());
    }

    #[test]
    fn single_bucket_spans_whole_signal() {
        let samples = vec![-0.5, 0.3, -0.9, 0.7, 0.1];
        let peaks = compute_peaks(&samples, 1);
        assert_eq!(peaks.len(), 1);
        assert_eq!(peaks[0].min, -0.9);
        assert_eq!(peaks[0].max, 0.7);
    }

    #[test]
    fn peaks_preserve_transient_spike() {
        // A flat-ish signal with one big spike in the middle bucket.
        let mut samples = vec![0.01_f32; 300];
        samples[150] = 0.95; // spike
        samples[151] = -0.93;
        let peaks = compute_peaks(&samples, 3);
        assert_eq!(peaks.len(), 3);
        // The middle bucket must capture the spike envelope.
        assert!(peaks[1].max >= 0.95 - 0.001);
        assert!(peaks[1].min <= -0.93 + 0.001);
    }

    #[test]
    fn bucket_count_clamped_to_sample_count() {
        let samples = vec![0.5, -0.5];
        let peaks = compute_peaks(&samples, 100);
        assert_eq!(peaks.len(), 2, "can't have more buckets than samples");
    }

    #[test]
    fn all_samples_covered_no_gaps() {
        // Ramp from -1 to 1; min of first bucket should be near -1,
        // max of last bucket near +1 — proving full coverage.
        let n = 1000;
        let samples: Vec<f32> = (0..n)
            .map(|i| -1.0 + 2.0 * (i as f32 / (n - 1) as f32))
            .collect();
        let peaks = compute_peaks(&samples, 10);
        assert_eq!(peaks.len(), 10);
        assert!(peaks[0].min <= -0.99);
        assert!(peaks[9].max >= 0.99);
    }

    #[test]
    fn levels_get_progressively_finer() {
        let samples: Vec<f32> = (0..10_000).map(|i| ((i as f32) * 0.01).sin()).collect();
        let wf = compute_levels(&samples, 16_000, 100, 4, 4);
        assert_eq!(wf.sample_rate, 16_000);
        assert_eq!(wf.total_samples, 10_000);
        assert!(wf.levels.len() >= 2);
        // Each level finer than the last (until clamped)
        assert_eq!(wf.levels[0].len(), 100);
        assert_eq!(wf.levels[1].len(), 400);
    }

    #[test]
    fn levels_stop_when_buckets_exceed_samples() {
        let samples = vec![0.1_f32; 50];
        let wf = compute_levels(&samples, 16_000, 100, 4, 10);
        // base_buckets (100) already > 50 samples → clamped to 50, stop.
        assert_eq!(wf.levels.len(), 1);
        assert_eq!(wf.levels[0].len(), 50);
    }

    // Round-trips a synthetic WAV through hound to exercise read_wav_samples
    // without needing ffmpeg or a real recording.
    #[test]
    fn reads_synthetic_wav() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tone.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        {
            let mut writer = hound::WavWriter::create(&path, spec).unwrap();
            // 0.5s of a 440 Hz tone
            for i in 0..8_000 {
                let t = i as f32 / 16_000.0;
                let v = (2.0 * std::f32::consts::PI * 440.0 * t).sin();
                writer.write_sample((v * i16::MAX as f32) as i16).unwrap();
            }
            writer.finalize().unwrap();
        }
        let (samples, sr) = read_wav_samples(&path).unwrap();
        assert_eq!(sr, 16_000);
        assert_eq!(samples.len(), 8_000);
        // A sine wave should swing close to ±1
        let max = samples.iter().cloned().fold(f32::MIN, f32::max);
        let min = samples.iter().cloned().fold(f32::MAX, f32::min);
        assert!(max > 0.9 && min < -0.9);
    }
}
