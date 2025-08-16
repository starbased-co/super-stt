// SPDX-License-Identifier: GPL-3.0-only
use spectrum_analyzer::scaling::divide_by_N_sqrt;
use spectrum_analyzer::windows::hann_window;
use spectrum_analyzer::{FrequencyLimit, FrequencySpectrum, samples_fft_to_spectrum};

/// Audio frequency analysis for wave visualization
#[derive(Debug, Clone)]
pub struct FrequencyData {
    pub bands: Vec<f32>, // Frequency band amplitudes (0.0 to 1.0)
    pub total_energy: f32,
    pub dominant_frequency: f32, // Dominant frequency in Hz for dynamic wave visualization
    pub frequency_confidence: f32, // Confidence of dominant frequency detection (0.0 to 1.0)
    pub dynamic_wave_frequency: Option<f32>, // Optional dynamic wave frequency for visualization
}

impl Default for FrequencyData {
    fn default() -> Self {
        Self {
            bands: vec![0.0; 64], // Default 64 frequency bands for richer visualization
            total_energy: 0.0,
            dominant_frequency: 440.0, // Default to A4 (440Hz) when no audio
            frequency_confidence: 0.0,
            dynamic_wave_frequency: None, // Let the applet handle wave frequency mapping
        }
    }
}

/// Number of frequency bands to compute for visualization
/// Using 64 bands provides richer visualization detail
const NUM_FREQUENCY_BANDS: usize = 64;

/// Audio analyzer that converts time-domain audio samples to frequency bands
#[derive(Debug, Clone)]
pub struct AudioAnalyzer {
    sample_rate: f32,
    buffer_size: usize,
}

impl AudioAnalyzer {
    #[must_use]
    pub fn new(sample_rate: f32, buffer_size: usize) -> Self {
        Self {
            sample_rate,
            buffer_size,
        }
    }

    /// Analyze audio samples and return frequency band amplitudes
    #[must_use]
    #[allow(
        clippy::cast_precision_loss,
        clippy::too_many_lines,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    pub fn analyze(&self, samples: &[f32]) -> FrequencyData {
        if samples.is_empty() {
            return FrequencyData {
                bands: vec![0.0; NUM_FREQUENCY_BANDS],
                total_energy: 0.0,
                dominant_frequency: 440.0, // Default A4
                frequency_confidence: 0.0,
                dynamic_wave_frequency: None,
            };
        }

        // Ensure we have enough samples for analysis
        let samples_to_use = if samples.len() < 64 {
            // Pad with zeros if too few samples
            let mut padded = samples.to_vec();
            padded.resize(64, 0.0);
            padded
        } else if samples.len() > self.buffer_size {
            // Take the most recent samples
            let start = samples.len() - self.buffer_size;
            samples[start..].to_vec()
        } else {
            samples.to_vec()
        };

        // Apply Hann window to reduce spectral leakage
        let windowed_samples = hann_window(&samples_to_use);

        // Perform FFT
        let spectrum_result = samples_fft_to_spectrum(
            &windowed_samples,
            self.sample_rate as u32,
            FrequencyLimit::All,
            Some(&divide_by_N_sqrt),
        );

        let spectrum = match spectrum_result {
            Ok(spectrum) => spectrum,
            Err(e) => {
                log::warn!("FFT analysis failed: {e}, returning zero bands");
                return FrequencyData {
                    bands: vec![0.0; NUM_FREQUENCY_BANDS],
                    total_energy: 0.0,
                    dominant_frequency: 440.0, // Default A4
                    frequency_confidence: 0.0,
                    dynamic_wave_frequency: None,
                };
            }
        };

        // Generate hybrid frequency bands: linear for low frequencies, logarithmic for high
        // This ensures good resolution where spectrum points are sparse (low freq)
        // and logarithmic spacing where human perception needs it (high freq)
        let mut band_amplitudes = Vec::with_capacity(NUM_FREQUENCY_BANDS);
        let mut total_energy = 0.0;

        // Define the transition point between linear and logarithmic spacing
        // Reduce linear band dominance by allocating fewer bands to low frequencies
        let linear_max_freq: f32 = 800.0; // Use linear spacing up to 800Hz (reduced from 1kHz)
        let log_min_freq: f32 = 800.0; // Start logarithmic spacing from 800Hz
        let log_max_freq: f32 = 16000.0; // End at 16kHz for extended range

        // Allocate bands: 20 for linear (50Hz-800Hz), 44 for logarithmic (800Hz-16kHz)
        // This gives more emphasis to mid-high frequencies where speech detail lies
        let linear_bands = (NUM_FREQUENCY_BANDS * 5) / 16; // ~20 bands
        let log_bands = NUM_FREQUENCY_BANDS - linear_bands; // ~44 bands

        // Generate linear frequency bands (50Hz - 800Hz)
        let linear_min_freq = 50.0;
        for i in 0..linear_bands {
            let t1 = i as f32 / linear_bands as f32;
            let t2 = (i + 1) as f32 / linear_bands as f32;

            let low_freq = linear_min_freq + t1 * (linear_max_freq - linear_min_freq);
            let high_freq = linear_min_freq + t2 * (linear_max_freq - linear_min_freq);

            let amplitude = self.calculate_band_amplitude(&spectrum, low_freq, high_freq);
            band_amplitudes.push(amplitude);
            total_energy += amplitude * amplitude;
        }

        // Generate logarithmic frequency bands (800Hz - 16kHz)
        let log_min = log_min_freq.ln();
        let log_max = log_max_freq.ln();

        for i in 0..log_bands {
            let t1 = i as f32 / log_bands as f32;
            let t2 = (i + 1) as f32 / log_bands as f32;

            let low_freq = (log_min + t1 * (log_max - log_min)).exp();
            let high_freq = (log_min + t2 * (log_max - log_min)).exp();

            let amplitude = self.calculate_band_amplitude(&spectrum, low_freq, high_freq);
            band_amplitudes.push(amplitude);
            total_energy += amplitude * amplitude;
        }

        // Calculate RMS total energy
        total_energy = (total_energy / NUM_FREQUENCY_BANDS as f32).sqrt();

        // Apply smart amplitude scaling with noise floor handling
        let input_rms = (samples_to_use.iter().map(|&x| x * x).sum::<f32>()
            / samples_to_use.len() as f32)
            .sqrt();

        // Determine noise floor dynamically
        let noise_floor_threshold = 0.0005; // Very low threshold for noise detection
        let quiet_threshold = 0.002; // Threshold for "quiet but real" audio
        let normal_threshold = 0.01; // Threshold for normal audio levels

        // Simple scaling without clamping - let natural differences show
        let scale_factor = if input_rms < noise_floor_threshold {
            5.0 // Very quiet scaling for noise
        } else if input_rms < quiet_threshold {
            25.0 // Light scaling for quiet audio
        } else if input_rms < normal_threshold {
            50.0 // Moderate scaling for normal audio
        } else {
            100.0 // Full scaling for loud audio
        };

        for band in &mut band_amplitudes {
            *band = (*band * scale_factor).sqrt();
        }

        // Apply frequency-specific balancing for better visualization
        if band_amplitudes.len() >= 64 {
            // Reduce dominance of low frequencies (first ~20 bands) by applying gentle dampening
            for i in 0..linear_bands {
                if i < band_amplitudes.len() {
                    // Apply progressive dampening: more dampening for lower frequencies
                    let damping_factor = 0.7 + (i as f32 / linear_bands as f32) * 0.3; // 0.7 to 1.0
                    band_amplitudes[i] *= damping_factor;
                }
            }

            // Boost mid-high frequencies (logarithmic bands) which are often weaker
            band_amplitudes
                .iter_mut()
                .skip(linear_bands)
                .take(20)
                .for_each(|v| *v *= 1.4); // 40% boost for mid frequencies

            // Additional boost for high frequencies which are typically very weak
            band_amplitudes
                .iter_mut()
                .skip(linear_bands + 20)
                .for_each(|v| *v *= 1.8); // 80% boost for high frequencies
        }

        // Extract dominant frequency for dynamic wave visualization
        let (dominant_frequency, frequency_confidence) =
            self.extract_dominant_frequency(&spectrum, &band_amplitudes);

        FrequencyData {
            bands: band_amplitudes,
            total_energy,
            dominant_frequency,
            frequency_confidence,
            dynamic_wave_frequency: None, // Let the applet handle wave frequency mapping
        }
    }

    /// Calculate amplitude for a frequency band using interpolation
    /// This ensures all bands get meaningful data even when spectrum points don't align perfectly
    #[allow(clippy::unused_self)]
    fn calculate_band_amplitude(
        &self,
        spectrum: &FrequencySpectrum,
        low_freq: f32,
        high_freq: f32,
    ) -> f32 {
        let mut weighted_sum = 0.0;
        let mut total_weight = 0.0;
        let band_center = f32::midpoint(low_freq, high_freq);
        let band_width = high_freq - low_freq;

        for (frequency, amplitude) in spectrum.data() {
            let freq_hz = frequency.val();
            let amp_val = amplitude.val();

            // Calculate how much this spectrum point contributes to our band
            let weight = self.calculate_frequency_weight(
                freq_hz,
                low_freq,
                high_freq,
                band_center,
                band_width,
            );

            if weight > 0.0 {
                weighted_sum += amp_val * weight;
                total_weight += weight;
            }
        }

        if total_weight > 0.0 {
            weighted_sum / total_weight
        } else {
            0.0
        }
    }

    /// Calculate how much a spectrum frequency contributes to a frequency band
    /// Uses a smooth weighting function to interpolate between spectrum points
    #[allow(clippy::unused_self)]
    fn calculate_frequency_weight(
        &self,
        freq: f32,
        band_low: f32,
        band_high: f32,
        band_center: f32,
        band_width: f32,
    ) -> f32 {
        if freq >= band_low && freq <= band_high {
            // Frequency is directly in the band - full weight
            1.0
        } else {
            // Calculate distance from band center
            let distance = (freq - band_center).abs();
            let max_influence = band_width * 1.5; // Allow influence beyond band edges

            if distance <= max_influence {
                // Use a smooth falloff function (Gaussian-like)
                let normalized_distance = distance / max_influence;
                (1.0 - normalized_distance * normalized_distance).max(0.0)
            } else {
                0.0
            }
        }
    }

    /// Extract the dominant frequency from the spectrum and frequency bands
    /// Returns (`frequency_hz`, `confidence_score`)
    fn extract_dominant_frequency(
        &self,
        spectrum: &FrequencySpectrum,
        band_amplitudes: &[f32],
    ) -> (f32, f32) {
        // Find the strongest frequency directly from the spectrum
        let mut max_amplitude = 0.0f32;
        let mut dominant_freq = 440.0f32; // Default to A4
        let mut total_energy = 0.0f32;

        // We'll analyze the spectrum in the typical speech frequency range (80Hz - 8kHz)
        // This avoids noise in very low/high frequencies and focuses on human speech
        for (frequency, amplitude) in spectrum.data() {
            let freq_hz = frequency.val();
            let amp_val = amplitude.val();

            // Focus on speech-relevant frequencies (80Hz - 8kHz)
            if (80.0..=8000.0).contains(&freq_hz) {
                total_energy += amp_val * amp_val;

                if amp_val > max_amplitude {
                    max_amplitude = amp_val;
                    dominant_freq = freq_hz;
                }
            }
        }

        // Calculate confidence based on how much the dominant frequency stands out
        // Confidence is higher when there's a clear peak vs distributed energy
        let confidence = if total_energy > 0.0 && max_amplitude > 0.0 {
            // Ratio of peak power to total power, normalized
            let peak_power = max_amplitude * max_amplitude;
            let peak_ratio = peak_power / total_energy;

            // Apply speech-specific weighting - mid frequencies get confidence boost
            let freq_weight = if (200.0..=2000.0).contains(&dominant_freq) {
                1.2 // Boost confidence for typical speech fundamentals
            } else if (80.0..=4000.0).contains(&dominant_freq) {
                1.0 // Normal confidence for extended speech range  
            } else {
                0.7 // Lower confidence for frequencies outside typical speech
            };

            // Scale and clamp confidence to 0.0-1.0
            (peak_ratio * freq_weight * 3.0).min(1.0)
        } else {
            0.0
        };

        // Apply smoothing to reduce rapid frequency jumps
        // For real-time visualization, we want some stability
        let smoothed_freq = if confidence < 0.3 {
            // Low confidence - fall back to analyzing frequency bands for general trends
            self.estimate_frequency_from_bands(band_amplitudes)
                .unwrap_or(440.0)
        } else {
            dominant_freq
        };

        (smoothed_freq, confidence)
    }

    /// Estimate dominant frequency from frequency bands when direct spectrum analysis is uncertain
    /// This provides a fallback method that looks at energy distribution across bands
    #[allow(clippy::unused_self, clippy::cast_precision_loss)]
    fn estimate_frequency_from_bands(&self, bands: &[f32]) -> Option<f32> {
        if bands.len() < 32 {
            return None;
        }

        // Find the band with maximum energy
        let mut max_energy = 0.0f32;
        let mut max_band_idx = 0;

        for (i, &energy) in bands.iter().enumerate() {
            if energy > max_energy {
                max_energy = energy;
                max_band_idx = i;
            }
        }

        // Convert band index to approximate frequency
        // Our bands are: first ~20 linear from 50-800Hz, then ~44 logarithmic from 800Hz-16kHz
        let linear_bands = (NUM_FREQUENCY_BANDS * 5) / 16; // ~20 bands

        let estimated_freq = if max_band_idx < linear_bands {
            // Linear frequency mapping (50Hz - 800Hz)
            let t = max_band_idx as f32 / linear_bands as f32;
            50.0 + t * (800.0 - 50.0)
        } else {
            // Logarithmic frequency mapping (800Hz - 16kHz)
            let log_bands = NUM_FREQUENCY_BANDS - linear_bands;
            let log_idx = max_band_idx - linear_bands;
            let t = log_idx as f32 / log_bands as f32;

            let log_min = 800.0f32.ln();
            let log_max = 16000.0f32.ln();
            (log_min + t * (log_max - log_min)).exp()
        };

        Some(estimated_freq)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_analyzer() {
        let analyzer = AudioAnalyzer::new(44100.0, 1024);

        // Generate a test tone at 440Hz (A4)
        let samples: Vec<f32> = (0..1024)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();

        let freq_data = analyzer.analyze(&samples);

        // Should have 64 frequency bands
        assert_eq!(freq_data.bands.len(), 64);

        // 440Hz should appear in one of the mid-range bands
        let mid_range_sum: f32 = freq_data.bands[16..48].iter().sum();
        assert!(
            mid_range_sum > 0.1,
            "440Hz tone should appear in mid-range bands"
        );

        // Should detect dominant frequency near 440Hz with reasonable confidence
        assert!(
            freq_data.dominant_frequency >= 400.0 && freq_data.dominant_frequency <= 480.0,
            "Dominant frequency should be near 440Hz, got {}",
            freq_data.dominant_frequency
        );
        assert!(
            freq_data.frequency_confidence > 0.0,
            "Should have some confidence in frequency detection"
        );
    }
}
