//! Deterministic sine-wave data with Gaussian noise and impulse spikes.

use std::f64::consts::TAU;

use rand::{Rng, SeedableRng, rngs::StdRng};
use rand_distr::{Distribution, Normal};

use crate::error::{ConfigError, ConfigResult};

/// Validated sine-wave parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SineConfig {
    samples: usize,
    amplitude: f64,
    cycles_per_sample: f64,
    phase: f64,
}

impl SineConfig {
    /// Construct a finite, non-empty sine-wave configuration.
    pub fn new(
        samples: usize,
        amplitude: f64,
        cycles_per_sample: f64,
        phase: f64,
    ) -> ConfigResult<Self> {
        if samples == 0 {
            return Err(ConfigError::InvalidValue {
                parameter: "samples",
                requirement: "must be greater than 0",
            });
        }
        for (parameter, value) in [
            ("amplitude", amplitude),
            ("frequency", cycles_per_sample),
            ("phase", phase),
        ] {
            if !value.is_finite() {
                return Err(ConfigError::NonFinite { parameter });
            }
        }
        if cycles_per_sample <= 0.0 {
            return Err(ConfigError::InvalidValue {
                parameter: "frequency",
                requirement: "must be greater than 0",
            });
        }

        Ok(Self {
            samples,
            amplitude,
            cycles_per_sample,
            phase,
        })
    }

    pub fn samples(self) -> usize {
        self.samples
    }

    pub fn amplitude(self) -> f64 {
        self.amplitude
    }

    pub fn cycles_per_sample(self) -> f64 {
        self.cycles_per_sample
    }

    pub fn phase(self) -> f64 {
        self.phase
    }
}

/// Validated synthetic noise parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoiseConfig {
    gaussian_sigma: f64,
    spike_probability: f64,
    spike_amplitude: f64,
}

impl NoiseConfig {
    /// Construct finite Gaussian and impulse-noise parameters.
    pub fn new(
        gaussian_sigma: f64,
        spike_probability: f64,
        spike_amplitude: f64,
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
}

/// Complete validated synthetic experiment configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SyntheticConfig {
    pub sine: SineConfig,
    pub noise: NoiseConfig,
    pub seed: u64,
}

/// Generated truth, measurements, and known impulse locations.
#[derive(Debug, Clone, PartialEq)]
pub struct SyntheticSeries {
    pub time: Vec<f64>,
    pub truth: Vec<f64>,
    pub noisy: Vec<f64>,
    pub spike_mask: Vec<bool>,
}

/// Generate the configured clean sine wave.
pub fn generate_truth(config: SineConfig) -> Vec<f64> {
    (0..config.samples)
        .map(|index| {
            config.amplitude * (TAU * config.cycles_per_sample * index as f64 + config.phase).sin()
        })
        .collect()
}

/// Add deterministic Gaussian noise and signed impulse spikes.
pub fn inject_noise(truth: &[f64], config: NoiseConfig, seed: u64) -> SyntheticSeries {
    let mut rng = StdRng::seed_from_u64(seed);
    let gaussian = (config.gaussian_sigma > 0.0)
        .then(|| Normal::new(0.0, config.gaussian_sigma).expect("validated positive sigma"));
    let mut noisy = Vec::with_capacity(truth.len());
    let mut spike_mask = Vec::with_capacity(truth.len());

    for &clean in truth {
        let gaussian_noise = gaussian
            .as_ref()
            .map_or(0.0, |distribution| distribution.sample(&mut rng));
        let is_spike = rng.random::<f64>() < config.spike_probability;
        let spike = if is_spike {
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

        noisy.push(clean + gaussian_noise + spike);
        spike_mask.push(is_spike);
    }

    SyntheticSeries {
        time: (0..truth.len()).map(|index| index as f64).collect(),
        truth: truth.to_vec(),
        noisy,
        spike_mask,
    }
}

/// Generate a complete deterministic synthetic series.
pub fn generate(config: SyntheticConfig) -> SyntheticSeries {
    let truth = generate_truth(config.sine);
    inject_noise(&truth, config.noise, config.seed)
}

#[cfg(test)]
mod tests {
    use super::{NoiseConfig, SineConfig, SyntheticConfig, generate, generate_truth, inject_noise};
    use crate::error::ConfigError;

    fn config(seed: u64) -> SyntheticConfig {
        SyntheticConfig {
            sine: SineConfig::new(64, 1.0, 0.03, 0.2).unwrap(),
            noise: NoiseConfig::new(0.2, 0.1, 2.0).unwrap(),
            seed,
        }
    }

    #[test]
    fn same_seed_produces_identical_vectors() {
        assert_eq!(generate(config(42)), generate(config(42)));
    }

    #[test]
    fn different_seeds_change_noisy_data() {
        assert_ne!(generate(config(1)).noisy, generate(config(2)).noisy);
    }

    #[test]
    fn disabled_noise_produces_exact_truth() {
        let sine = SineConfig::new(16, 2.0, 0.1, 0.0).unwrap();
        let series = generate(SyntheticConfig {
            sine,
            noise: NoiseConfig::new(0.0, 0.0, 5.0).unwrap(),
            seed: 9,
        });

        assert_eq!(series.noisy, series.truth);
        assert!(series.spike_mask.iter().all(|is_spike| !is_spike));
    }

    #[test]
    fn probability_one_marks_every_sample() {
        let truth = vec![0.0; 12];
        let series = inject_noise(&truth, NoiseConfig::new(0.0, 1.0, 1.0).unwrap(), 12);

        assert!(series.spike_mask.iter().all(|is_spike| *is_spike));
    }

    #[test]
    fn truth_matches_known_quarter_cycle_values() {
        let values = generate_truth(SineConfig::new(4, 2.0, 0.25, 0.0).unwrap());

        assert!((values[0] - 0.0).abs() < 1e-12);
        assert!((values[1] - 2.0).abs() < 1e-12);
        assert!(values[2].abs() < 1e-12);
        assert!((values[3] + 2.0).abs() < 1e-12);
    }

    #[test]
    fn invalid_parameters_fail() {
        assert!(matches!(
            SineConfig::new(0, 1.0, 0.1, 0.0),
            Err(ConfigError::InvalidValue {
                parameter: "samples",
                ..
            })
        ));
        assert!(SineConfig::new(1, f64::NAN, 0.1, 0.0).is_err());
        assert!(SineConfig::new(1, 1.0, 0.0, 0.0).is_err());
        assert!(NoiseConfig::new(-0.1, 0.0, 1.0).is_err());
        assert!(NoiseConfig::new(0.1, 1.1, 1.0).is_err());
        assert!(NoiseConfig::new(0.1, 0.1, f64::INFINITY).is_err());
    }
}
