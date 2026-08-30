//! Spike-region error and recovery metrics when a spike mask is known.

use crate::error::{ConfigError, ConfigResult};

use super::{MetricsError, error, validate_pair};

/// Recovery threshold and stability requirements.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RecoveryConfig {
    tolerance: f64,
    stable_count: usize,
}

impl RecoveryConfig {
    pub fn new(tolerance: f64, stable_count: usize) -> ConfigResult<Self> {
        if !tolerance.is_finite() {
            return Err(ConfigError::NonFinite {
                parameter: "recovery_tolerance",
            });
        }
        if tolerance < 0.0 {
            return Err(ConfigError::InvalidValue {
                parameter: "recovery_tolerance",
                requirement: "must be at least 0",
            });
        }
        if stable_count == 0 {
            return Err(ConfigError::InvalidValue {
                parameter: "stable_count",
                requirement: "must be greater than 0",
            });
        }

        Ok(Self {
            tolerance,
            stable_count,
        })
    }
}

/// Error at known spike positions and post-spike recovery lengths.
#[derive(Debug, Clone, PartialEq)]
pub struct SpikeMetrics {
    pub count: usize,
    pub spike_rmse: Option<f64>,
    pub recovered_count: usize,
    pub unrecovered_count: usize,
    pub mean_recovery_samples: Option<f64>,
    pub max_recovery_samples: Option<usize>,
}

/// Compute spike-only RMSE and recovery statistics.
pub fn compute(
    reference: &[f64],
    estimate: &[f64],
    spike_mask: &[bool],
    config: RecoveryConfig,
) -> Result<SpikeMetrics, MetricsError> {
    validate_pair(reference, estimate)?;
    if spike_mask.len() != reference.len() {
        return Err(MetricsError::MaskLengthMismatch {
            expected: reference.len(),
            actual: spike_mask.len(),
        });
    }

    let spike_indices: Vec<_> = spike_mask
        .iter()
        .enumerate()
        .filter_map(|(index, &is_spike)| is_spike.then_some(index))
        .collect();
    if spike_indices.is_empty() {
        return Ok(SpikeMetrics {
            count: 0,
            spike_rmse: None,
            recovered_count: 0,
            unrecovered_count: 0,
            mean_recovery_samples: None,
            max_recovery_samples: None,
        });
    }

    let spike_reference: Vec<_> = spike_indices
        .iter()
        .map(|&index| reference[index])
        .collect();
    let spike_estimate: Vec<_> = spike_indices.iter().map(|&index| estimate[index]).collect();
    let spike_rmse = error::compute(&spike_reference, &spike_estimate)?.rmse;

    let mut recovery_lengths = Vec::new();
    let mut unrecovered_count = 0;
    for &spike_index in &spike_indices {
        if let Some(length) = recovery_length(reference, estimate, spike_index, config) {
            recovery_lengths.push(length);
        } else {
            unrecovered_count += 1;
        }
    }

    let mean_recovery_samples = (!recovery_lengths.is_empty())
        .then(|| recovery_lengths.iter().sum::<usize>() as f64 / recovery_lengths.len() as f64);
    let max_recovery_samples = recovery_lengths.iter().copied().max();

    Ok(SpikeMetrics {
        count: spike_indices.len(),
        spike_rmse: Some(spike_rmse),
        recovered_count: recovery_lengths.len(),
        unrecovered_count,
        mean_recovery_samples,
        max_recovery_samples,
    })
}

fn recovery_length(
    reference: &[f64],
    estimate: &[f64],
    spike_index: usize,
    config: RecoveryConfig,
) -> Option<usize> {
    let first = spike_index + 1;
    if first + config.stable_count > reference.len() {
        return None;
    }

    (first..=reference.len() - config.stable_count).find_map(|start| {
        let stable = (start..start + config.stable_count)
            .all(|index| (estimate[index] - reference[index]).abs() <= config.tolerance);
        stable.then_some(start - spike_index)
    })
}

#[cfg(test)]
mod tests {
    use super::{RecoveryConfig, compute};
    use crate::metrics::MetricsError;

    #[test]
    fn no_spikes_returns_empty_optional_metrics() {
        let metrics = compute(
            &[0.0, 0.0],
            &[1.0, 0.0],
            &[false, false],
            RecoveryConfig::new(0.1, 1).unwrap(),
        )
        .unwrap();

        assert_eq!(metrics.count, 0);
        assert_eq!(metrics.spike_rmse, None);
        assert_eq!(metrics.mean_recovery_samples, None);
    }

    #[test]
    fn computes_spike_error_and_recovery() {
        let metrics = compute(
            &[0.0; 6],
            &[0.0, 4.0, 2.0, 0.05, 0.0, 0.0],
            &[false, true, false, false, false, false],
            RecoveryConfig::new(0.1, 2).unwrap(),
        )
        .unwrap();

        assert_eq!(metrics.spike_rmse, Some(4.0));
        assert_eq!(metrics.recovered_count, 1);
        assert_eq!(metrics.unrecovered_count, 0);
        assert_eq!(metrics.mean_recovery_samples, Some(2.0));
        assert_eq!(metrics.max_recovery_samples, Some(2));
    }

    #[test]
    fn reports_unrecovered_spikes_and_bad_mask() {
        let metrics = compute(
            &[0.0, 0.0],
            &[1.0, 1.0],
            &[true, false],
            RecoveryConfig::new(0.1, 1).unwrap(),
        )
        .unwrap();
        assert_eq!(metrics.unrecovered_count, 1);

        assert!(matches!(
            compute(&[0.0], &[0.0], &[], RecoveryConfig::new(0.1, 1).unwrap()),
            Err(MetricsError::MaskLengthMismatch { .. })
        ));
    }

    #[test]
    fn validates_recovery_config() {
        assert!(RecoveryConfig::new(f64::NAN, 1).is_err());
        assert!(RecoveryConfig::new(-0.1, 1).is_err());
        assert!(RecoveryConfig::new(0.1, 0).is_err());
    }
}
