//! Formatting-independent error, SNR, and spike metrics.

use thiserror::Error;

pub mod error;
pub mod snr;
pub mod spike;

/// Invalid metric inputs or unrepresentable arithmetic.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum MetricsError {
    #[error("metric input series must not be empty")]
    EmptySeries,

    #[error("metric series length mismatch: reference has {reference}, estimate has {estimate}")]
    LengthMismatch { reference: usize, estimate: usize },

    #[error("spike mask length mismatch: expected {expected}, got {actual}")]
    MaskLengthMismatch { expected: usize, actual: usize },

    #[error("non-finite value in {series} at index {index}")]
    NonFiniteValue { series: &'static str, index: usize },

    #[error("metric arithmetic produced a non-finite result")]
    NumericalFailure,
}

pub(crate) fn validate_pair(reference: &[f64], estimate: &[f64]) -> Result<(), MetricsError> {
    if reference.is_empty() {
        return Err(MetricsError::EmptySeries);
    }
    if reference.len() != estimate.len() {
        return Err(MetricsError::LengthMismatch {
            reference: reference.len(),
            estimate: estimate.len(),
        });
    }
    for (series, values) in [("reference", reference), ("estimate", estimate)] {
        if let Some((index, _)) = values
            .iter()
            .enumerate()
            .find(|(_, value)| !value.is_finite())
        {
            return Err(MetricsError::NonFiniteValue { series, index });
        }
    }
    Ok(())
}

pub(crate) fn root_mean_square(values: impl Iterator<Item = f64>, count: usize) -> f64 {
    let mut scale = 0.0;
    let mut scaled_sum = 0.0;

    for value in values {
        let absolute = value.abs();
        if absolute == 0.0 {
            continue;
        }
        if scale < absolute {
            scaled_sum = 1.0 + scaled_sum * (scale / absolute).powi(2);
            scale = absolute;
        } else {
            scaled_sum += (absolute / scale).powi(2);
        }
    }

    if scale == 0.0 {
        0.0
    } else {
        scale * (scaled_sum / count as f64).sqrt()
    }
}
