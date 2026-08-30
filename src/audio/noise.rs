//! Deterministic Gaussian and impulse noise over interleaved audio samples.

use rand::{Rng, SeedableRng, rngs::StdRng};
use rand_distr::{Distribution, Normal};
use thiserror::Error;

use crate::{
    audio::wav::AudioBuffer,
    error::{ConfigError, ConfigResult},
};

/// Validated audio noise parameters and seed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioNoiseConfig {
    gaussian_sigma: f64,
    spike_probability: f64,
    spike_amplitude: f64,
    seed: u64,
}

impl AudioNoiseConfig {
    pub fn new(
        gaussian_sigma: f64,
        spike_probability: f64,
        spike_amplitude: f64,
        seed: u64,
    ) -> ConfigResult<Self> {
        for (parameter, value) in [
            ("gaussian_sigma", gaussian_sigma),
            ("spike_probability", spike_probability),
            ("spike_amplitude", spike_amplitude),
        ] {
            if !value.is_finite() {
                return Err(ConfigError::NonFinite { parameter });
            }
        }
        if gaussian_sigma < 0.0 {
            return Err(ConfigError::InvalidValue {
                parameter: "gaussian_sigma",
                requirement: "must be at least 0",
            });
        }
        if !(0.0..=1.0).contains(&spike_probability) {
            return Err(ConfigError::InvalidValue {
                parameter: "spike_probability",
                requirement: "must be between 0 and 1 inclusive",
            });
        }
        if spike_amplitude < 0.0 {
            return Err(ConfigError::InvalidValue {
                parameter: "spike_amplitude",
                requirement: "must be at least 0",
            });
        }

        Ok(Self {
            gaussian_sigma,
            spike_probability,
            spike_amplitude,
            seed,
        })
    }

    pub fn gaussian_sigma(self) -> f64 {
        self.gaussian_sigma
    }

    pub fn spike_probability(self) -> f64 {
        self.spike_probability
    }

    pub fn spike_amplitude(self) -> f64 {
        self.spike_amplitude
    }

    pub fn seed(self) -> u64 {
        self.seed
    }
}

/// Noisy interleaved samples and one known-spike flag per sample/channel.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioNoiseResult {
    pub samples: Vec<f64>,
    pub spike_mask: Vec<bool>,
}

/// Failures caused by unrepresentable finite noise arithmetic.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AudioNoiseError {
    #[error("audio noise injection produced a non-finite sample at interleaved index {index}")]
    NumericalFailure { index: usize },
}

/// Inject deterministic noise using one RNG stream in interleaved sample order.
///
/// Changing channel count changes RNG consumption and therefore the realization.
pub fn inject_noise(
    audio: &AudioBuffer,
    config: AudioNoiseConfig,
) -> Result<AudioNoiseResult, AudioNoiseError> {
    let mut rng = StdRng::seed_from_u64(config.seed);
    let gaussian = (config.gaussian_sigma > 0.0)
        .then(|| Normal::new(0.0, config.gaussian_sigma).expect("validated positive sigma"));
    let mut samples = Vec::with_capacity(audio.interleaved.len());
    let mut spike_mask = Vec::with_capacity(audio.interleaved.len());

    for (index, &clean) in audio.interleaved.iter().enumerate() {
        let gaussian_noise = gaussian
            .as_ref()
            .map_or(0.0, |distribution| distribution.sample(&mut rng));
        let is_spike = rng.random::<f64>() < config.spike_probability;
        let spike = if is_spike && config.spike_amplitude > 0.0 {
            let magnitude =
                rng.random_range((0.5 * config.spike_amplitude)..=config.spike_amplitude);
            if rng.random_bool(0.5) {
                magnitude
            } else {
                -magnitude
            }
        } else {
            0.0
        };
        let noisy = clean + gaussian_noise + spike;
        if !noisy.is_finite() {
            return Err(AudioNoiseError::NumericalFailure { index });
        }

        samples.push(noisy);
        spike_mask.push(is_spike);
    }

    Ok(AudioNoiseResult {
        samples,
        spike_mask,
    })
}

#[cfg(test)]
mod tests {
    use super::{AudioNoiseConfig, inject_noise};
    use crate::audio::wav::AudioBuffer;

    fn audio() -> AudioBuffer {
        AudioBuffer::new(16_000, 2, vec![0.0, 0.25, -0.25, 0.5]).unwrap()
    }

    #[test]
    fn seeded_injection_is_deterministic() {
        let config = AudioNoiseConfig::new(0.1, 0.5, 0.8, 42).unwrap();

        assert_eq!(
            inject_noise(&audio(), config).unwrap(),
            inject_noise(&audio(), config).unwrap()
        );
    }

    #[test]
    fn zero_noise_returns_exact_samples() {
        let audio = audio();
        let result =
            inject_noise(&audio, AudioNoiseConfig::new(0.0, 0.0, 0.0, 1).unwrap()).unwrap();

        assert_eq!(result.samples, audio.interleaved);
        assert!(result.spike_mask.iter().all(|is_spike| !is_spike));
    }

    #[test]
    fn probability_one_marks_every_interleaved_sample() {
        let result =
            inject_noise(&audio(), AudioNoiseConfig::new(0.0, 1.0, 0.5, 2).unwrap()).unwrap();

        assert_eq!(result.spike_mask.len(), 4);
        assert!(result.spike_mask.iter().all(|is_spike| *is_spike));
    }

    #[test]
    fn internal_samples_are_not_clamped() {
        let loud = AudioBuffer::new(16_000, 1, vec![1.0; 16]).unwrap();
        let result = inject_noise(&loud, AudioNoiseConfig::new(0.0, 1.0, 1.0, 3).unwrap()).unwrap();

        assert!(result.samples.iter().any(|sample| sample.abs() > 1.0));
    }

    #[test]
    fn validates_configuration() {
        assert!(AudioNoiseConfig::new(f64::NAN, 0.0, 0.0, 1).is_err());
        assert!(AudioNoiseConfig::new(-0.1, 0.0, 0.0, 1).is_err());
        assert!(AudioNoiseConfig::new(0.0, 1.1, 0.0, 1).is_err());
        assert!(AudioNoiseConfig::new(0.0, 0.0, -1.0, 1).is_err());
    }
}
