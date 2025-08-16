// SPDX-License-Identifier: GPL-3.0-only

use anyhow::Result;
use cpal::Device;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub const BEEP_FADE_IN_MS: u64 = 20;
pub const WARMUP_TONE_DURATION_MS: u64 = 30;
pub const WARMUP_TONE_FREQUENCY: f32 = 44000.0;
pub const WARMUP_DELAY_AFTER_TONE_MS: u64 = 300;

/// Play a short warm-up tone to initialize audio drivers.
///
/// # Errors
///
/// Returns an error only if the post-tone sleep fails (unlikely). Errors
/// from the tone itself are logged and ignored.
pub fn play_warmup_tone() -> Result<()> {
    log::debug!("Playing warm-up tone to initialize audio drivers");
    let warmup_frequencies = [WARMUP_TONE_FREQUENCY];
    let warmup_duration = WARMUP_TONE_DURATION_MS;
    if let Err(e) = play_beep_sequence(&warmup_frequencies, warmup_duration) {
        log::debug!("Warm-up tone failed (usually fine): {e}");
    }
    std::thread::sleep(Duration::from_millis(WARMUP_DELAY_AFTER_TONE_MS));
    Ok(())
}

/// Play a sequence of beeps on a freshly initialized output device.
///
/// # Errors
///
/// Returns an error if no output device is available or if the output stream
/// cannot be created or played.
#[allow(
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub fn play_beep_sequence(frequencies: &[f32], duration_ms: u64) -> Result<()> {
    if frequencies.is_empty() {
        return Ok(());
    }

    log::info!(
        "Playing beep sequence with fresh device initialization: {frequencies:?} for {duration_ms}ms each"
    );

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow::anyhow!("No output device available"))?;

    let config = device
        .default_output_config()
        .map_err(|e| anyhow::anyhow!("Failed to get output config: {}", e))?;

    let sample_rate = config.sample_rate().0 as f32;
    let channels = config.channels() as usize;

    log::debug!(
        "Fresh audio device: sample_rate={}Hz, channels={}, format={:?}",
        sample_rate,
        channels,
        config.sample_format()
    );

    if let Err(e) = verify_fresh_device(&device, &config) {
        log::warn!("Device verification failed, continuing anyway: {e}");
    }

    for (beep_index, &frequency) in frequencies.iter().enumerate() {
        let samples_per_beep = (sample_rate * duration_ms as f32 / 1000.0) as usize;
        let fade_in_samples = (sample_rate * BEEP_FADE_IN_MS as f32 / 1000.0) as usize;
        let mut sample_clock = 0usize;
        let finished = std::sync::Arc::new(AtomicBool::new(false));
        let finished_clone = finished.clone();

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => device.build_output_stream(
                &config.config(),
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    for frame in data.chunks_mut(channels) {
                        if sample_clock >= samples_per_beep {
                            finished_clone.store(true, Ordering::Relaxed);
                            for sample in frame {
                                *sample = 0.0;
                            }
                            return;
                        }
                        let fade_multiplier = if sample_clock < fade_in_samples {
                            sample_clock as f32 / fade_in_samples as f32
                        } else {
                            1.0
                        };
                        let value = (sample_clock as f32 * frequency * 2.0 * std::f32::consts::PI
                            / sample_rate)
                            .sin()
                            * 0.3
                            * fade_multiplier;
                        for sample in frame {
                            *sample = value;
                        }
                        sample_clock += 1;
                    }
                },
                |err| log::warn!("Audio stream error: {err}"),
                None,
            ),
            cpal::SampleFormat::I16 => device.build_output_stream(
                &config.config(),
                move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                    for frame in data.chunks_mut(channels) {
                        if sample_clock >= samples_per_beep {
                            finished_clone.store(true, Ordering::Relaxed);
                            for sample in frame {
                                *sample = 0;
                            }
                            return;
                        }
                        let fade_multiplier = if sample_clock < fade_in_samples {
                            sample_clock as f32 / fade_in_samples as f32
                        } else {
                            1.0
                        };
                        let value = (sample_clock as f32 * frequency * 2.0 * std::f32::consts::PI
                            / sample_rate)
                            .sin()
                            * 0.3
                            * fade_multiplier;
                        let sample_value = (value * f32::from(i16::MAX)) as i16;
                        for sample in frame {
                            *sample = sample_value;
                        }
                        sample_clock += 1;
                    }
                },
                |err| log::warn!("Audio stream error: {err}"),
                None,
            ),
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported sample format for beep playback"
                ));
            }
        }?;

        if beep_index == 0 {
            std::thread::sleep(Duration::from_millis(150));
        }

        stream
            .play()
            .map_err(|e| anyhow::anyhow!("Failed to play beep: {}", e))?;

        let beep_start = std::time::Instant::now();
        let beep_timeout = Duration::from_millis(duration_ms + 1000);
        while !finished.load(Ordering::Relaxed) {
            if beep_start.elapsed() > beep_timeout {
                log::warn!("Beep playback timed out");
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        drop(stream);
        std::thread::sleep(Duration::from_millis(50));
    }

    Ok(())
}

/// Verify the output device by opening a short silent stream.
///
/// # Errors
///
/// Returns an error if a test stream cannot be created or played.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub fn verify_fresh_device(device: &Device, config: &cpal::SupportedStreamConfig) -> Result<()> {
    let sample_rate = config.sample_rate().0 as f32;
    let channels = config.channels() as usize;
    let verification_samples = (sample_rate * 0.05) as usize; // 50ms of silence
    let mut sample_count = 0usize;
    let completed = std::sync::Arc::new(AtomicBool::new(false));
    let completed_clone = completed.clone();

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_output_stream(
            &config.config(),
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                for frame in data.chunks_mut(channels) {
                    if sample_count >= verification_samples {
                        completed_clone.store(true, Ordering::Relaxed);
                        return;
                    }
                    for sample in frame {
                        *sample = 0.0;
                    }
                    sample_count += 1;
                }
            },
            |err| log::debug!("Verification stream error: {err}"),
            None,
        )?,
        cpal::SampleFormat::I16 => device.build_output_stream(
            &config.config(),
            move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                for frame in data.chunks_mut(channels) {
                    if sample_count >= verification_samples {
                        completed_clone.store(true, Ordering::Relaxed);
                        return;
                    }
                    for sample in frame {
                        *sample = 0;
                    }
                    sample_count += 1;
                }
            },
            |err| log::debug!("Verification stream error: {err}"),
            None,
        )?,
        _ => return Err(anyhow::anyhow!("Unsupported sample format")),
    };

    stream.play()?;
    let start = std::time::Instant::now();
    while !completed.load(Ordering::Relaxed) && start.elapsed() < Duration::from_millis(200) {
        std::thread::sleep(Duration::from_millis(5));
    }
    drop(stream);
    Ok(())
}
