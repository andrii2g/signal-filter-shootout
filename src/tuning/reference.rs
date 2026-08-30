//! Offline Hampel-style despiking and forward/backward EWMA reference construction.

use crate::{
    error::{ConfigError, ConfigResult},
    filters::{
        FilterError, OnlineFilter,
        ewma::{EwmaConfig, EwmaFilter},
    },
};
use thiserror::Error;

/// Validated offline reference parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReferenceConfig {
    hampel_radius: usize,
    threshold: f64,
    ewma: EwmaConfig,
}

impl ReferenceConfig {
    pub fn new(hampel_radius: usize, threshold: f64, ewma_alpha: f64) -> ConfigResult<Self> {
        if hampel_radius == 0 {
            return Err(ConfigError::InvalidValue {
                parameter: "hampel_radius",
                requirement: "must be greater than 0",
            });
        }
        if !threshold.is_finite() {
            return Err(ConfigError::NonFinite {
                parameter: "hampel_threshold",
            });
        }
        if threshold <= 0.0 {
            return Err(ConfigError::InvalidValue {
                parameter: "hampel_threshold",
                requirement: "must be greater than 0",
            });
        }

        Ok(Self {
            hampel_radius,
            threshold,
            ewma: EwmaConfig::new(ewma_alpha)?,
        })
    }

    pub fn hampel_radius(self) -> usize {
        self.hampel_radius
    }

    pub fn threshold(self) -> f64 {
        self.threshold
    }

    pub fn ewma_alpha(self) -> f64 {
        self.ewma.alpha()
    }
}

impl Default for ReferenceConfig {
    fn default() -> Self {
        Self::new(3, 3.0, 0.10).expect("valid default reference configuration")
    }
}

/// Failures while constructing an offline pseudo-reference.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReferenceError {
    #[error("pseudo-reference input must not be empty")]
    Empty,

    #[error("pseudo-reference input contains a non-finite value at index {index}")]
    NonFiniteInput { index: usize },

    #[error(transparent)]
    Filter(#[from] FilterError),
}

/// Replace local Hampel outliers without smoothing ordinary samples.
pub fn hampel_despike(values: &[f64], config: ReferenceConfig) -> Result<Vec<f64>, ReferenceError> {
    validate_input(values)?;
    let mut output = Vec::with_capacity(values.len());

    for (index, &value) in values.iter().enumerate() {
        let start = index.saturating_sub(config.hampel_radius);
        let end = (index + config.hampel_radius + 1).min(values.len());
        let window = &values[start..end];
        let local_median = median(window);
        let deviations: Vec<_> = window
            .iter()
            .map(|sample| (sample - local_median).abs())
            .collect();
        let mad = median(&deviations);
        let robust_sigma = 1.4826 * mad;
        let deviation = (value - local_median).abs();

        let replace = if robust_sigma > 0.0 {
            deviation > config.threshold * robust_sigma
        } else {
            let epsilon = 1e-12 * local_median.abs().max(1.0);
            let constant_neighbors = values[start..end]
                .iter()
                .enumerate()
                .filter(|(offset, _)| start + offset != index)
                .all(|(_, sample)| (sample - local_median).abs() <= epsilon);
            constant_neighbors && deviation > epsilon
        };
        output.push(if replace { local_median } else { value });
    }

    Ok(output)
}

/// Apply EWMA forward and backward to approximate zero-phase smoothing.
pub fn forward_backward_ewma(
    values: &[f64],
    config: EwmaConfig,
) -> Result<Vec<f64>, ReferenceError> {
    validate_input(values)?;
    let forward = filter_values(values.iter().copied(), config)?;
    let mut backward = filter_values(forward.iter().rev().copied(), config)?;
    backward.reverse();
    Ok(backward)
}

/// Build the complete offline pseudo-reference.
pub fn build_pseudo_reference(
    values: &[f64],
    config: ReferenceConfig,
) -> Result<Vec<f64>, ReferenceError> {
    let despiked = hampel_despike(values, config)?;
    forward_backward_ewma(&despiked, config.ewma)
}

fn filter_values(
    values: impl Iterator<Item = f64>,
    config: EwmaConfig,
) -> Result<Vec<f64>, FilterError> {
    let mut filter = EwmaFilter::new(config);
    values.map(|value| filter.update(value)).collect()
}

fn validate_input(values: &[f64]) -> Result<(), ReferenceError> {
    if values.is_empty() {
        return Err(ReferenceError::Empty);
    }
    if let Some((index, _)) = values
        .iter()
        .enumerate()
        .find(|(_, value)| !value.is_finite())
    {
        return Err(ReferenceError::NonFiniteInput { index });
    }
    Ok(())
}

fn median(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        sorted[middle - 1] / 2.0 + sorted[middle] / 2.0
    } else {
        sorted[middle]
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ReferenceConfig, ReferenceError, build_pseudo_reference, forward_backward_ewma,
        hampel_despike,
    };
    use crate::filters::ewma::EwmaConfig;

    #[test]
    fn constant_input_remains_constant() {
        let reference = build_pseudo_reference(&[4.0; 20], ReferenceConfig::default()).unwrap();

        assert_eq!(reference, [4.0; 20]);
    }

    #[test]
    fn isolated_extreme_spike_is_replaced() {
        let values = [1.0, 1.0, 1.0, 1000.0, 1.0, 1.0, 1.0];
        let despiked = hampel_despike(&values, ReferenceConfig::default()).unwrap();

        assert_eq!(despiked, [1.0; 7]);
    }

    #[test]
    fn forward_backward_smoothing_preserves_sample_count() {
        let values = [0.0, 1.0, 0.0, 1.0, 0.0];
        let smoothed = forward_backward_ewma(&values, EwmaConfig::new(0.1).unwrap()).unwrap();

        assert_eq!(smoothed.len(), values.len());
        assert!(smoothed.iter().all(|value| value.is_finite()));
    }

    #[test]
    fn output_is_deterministic() {
        let values = [0.0, 0.5, 10.0, -0.5, 0.0];

        assert_eq!(
            build_pseudo_reference(&values, ReferenceConfig::default()).unwrap(),
            build_pseudo_reference(&values, ReferenceConfig::default()).unwrap()
        );
    }

    #[test]
    fn validates_configuration_and_input() {
        assert!(ReferenceConfig::new(0, 3.0, 0.1).is_err());
        assert!(ReferenceConfig::new(3, 0.0, 0.1).is_err());
        assert!(ReferenceConfig::new(3, 3.0, f64::NAN).is_err());
        assert_eq!(
            build_pseudo_reference(&[], ReferenceConfig::default()),
            Err(ReferenceError::Empty)
        );
        assert!(matches!(
            build_pseudo_reference(&[1.0, f64::NAN], ReferenceConfig::default()),
            Err(ReferenceError::NonFiniteInput { index: 1 })
        ));
    }
}
