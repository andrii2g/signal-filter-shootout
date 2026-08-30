//! Online scalar filters independent of CLI, signal, and audio concerns.

use thiserror::Error;

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

pub(crate) fn validate_measurement(measurement: f64) -> Result<(), FilterError> {
    if measurement.is_finite() {
        Ok(())
    } else {
        Err(FilterError::NonFiniteMeasurement)
    }
}
