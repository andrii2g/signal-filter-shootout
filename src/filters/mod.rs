//! Online scalar filters independent of CLI, signal, and audio concerns.

use thiserror::Error;

use self::{
    ewma::{EwmaConfig, EwmaFilter},
    kalman::{KalmanConfig, ScalarKalmanFilter},
    median::{MedianConfig, MedianFilter},
};

pub mod ewma;
pub mod kalman;
pub mod median;

/// Runtime failures shared by online filters.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum FilterError {
    /// The caller supplied NaN or infinity as a measurement.
    #[error("filter measurement must be finite")]
    NonFiniteMeasurement,

    /// Finite inputs nevertheless exceeded representable scalar arithmetic.
    #[error("filter update produced a non-finite numerical result")]
    NumericalFailure,
}

/// Common interface for stateful, sample-by-sample scalar filters.
pub trait OnlineFilter {
    /// Restore the filter to its freshly constructed state.
    fn reset(&mut self);

    /// Consume one finite measurement and produce one filtered sample.
    fn update(&mut self, measurement: f64) -> Result<f64, FilterError>;
}

/// Validated parameters for the three-filter shootout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShootoutConfig {
    pub ewma: EwmaConfig,
    pub median: MedianConfig,
    pub kalman: KalmanConfig,
}

/// One output sample per input sample for every filter.
#[derive(Debug, Clone, PartialEq)]
pub struct FilterOutputs {
    pub raw: Vec<f64>,
    pub ewma: Vec<f64>,
    pub median: Vec<f64>,
    pub kalman: Vec<f64>,
}

/// Apply all three filters independently to the same measurements.
pub fn apply_all(
    measurements: &[f64],
    config: ShootoutConfig,
) -> Result<FilterOutputs, FilterError> {
    Ok(FilterOutputs {
        raw: measurements.to_vec(),
        ewma: apply_filter(EwmaFilter::new(config.ewma), measurements)?,
        median: apply_filter(MedianFilter::new(config.median), measurements)?,
        kalman: apply_filter(ScalarKalmanFilter::new(config.kalman), measurements)?,
    })
}

fn apply_filter(
    mut filter: impl OnlineFilter,
    measurements: &[f64],
) -> Result<Vec<f64>, FilterError> {
    measurements
        .iter()
        .map(|&measurement| filter.update(measurement))
        .collect()
}

pub(crate) fn validate_measurement(measurement: f64) -> Result<(), FilterError> {
    if measurement.is_finite() {
        Ok(())
    } else {
        Err(FilterError::NonFiniteMeasurement)
    }
}

#[cfg(test)]
mod tests {
    use super::{ShootoutConfig, apply_all};
    use crate::filters::{ewma::EwmaConfig, kalman::KalmanConfig, median::MedianConfig};

    #[test]
    fn all_outputs_preserve_the_input_sample_count() {
        let input = [1.0, -2.0, 3.0, 4.0];
        let outputs = apply_all(
            &input,
            ShootoutConfig {
                ewma: EwmaConfig::new(1.0).unwrap(),
                median: MedianConfig::new(1).unwrap(),
                kalman: KalmanConfig::new(0.1, 1.0, 1.0).unwrap(),
            },
        )
        .unwrap();

        assert_eq!(outputs.raw, input);
        assert_eq!(outputs.ewma, input);
        assert_eq!(outputs.median, input);
        assert_eq!(outputs.kalman.len(), input.len());
        assert_eq!(outputs.kalman[0], input[0]);
    }

    #[test]
    fn invalid_measurement_fails_the_shootout() {
        let result = apply_all(
            &[1.0, f64::NAN],
            ShootoutConfig {
                ewma: EwmaConfig::new(0.2).unwrap(),
                median: MedianConfig::new(3).unwrap(),
                kalman: KalmanConfig::new(0.1, 1.0, 1.0).unwrap(),
            },
        );

        assert!(result.is_err());
    }
}
