//! RMSE, MAE, and maximum absolute error.

use super::{MetricsError, root_mean_square, validate_pair};

/// Basic pointwise error statistics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ErrorMetrics {
    pub rmse: f64,
    pub mae: f64,
    pub max_abs: f64,
}

/// Compute basic errors for two finite, equally sized non-empty series.
pub fn compute(reference: &[f64], estimate: &[f64]) -> Result<ErrorMetrics, MetricsError> {
    validate_pair(reference, estimate)?;

    let mut errors = Vec::with_capacity(reference.len());
    let mut mae = 0.0;
    let mut max_abs: f64 = 0.0;

    for (index, (&reference, &estimate)) in reference.iter().zip(estimate).enumerate() {
        let error = estimate - reference;
        if !error.is_finite() {
            return Err(MetricsError::NonFiniteValue {
                series: "error",
                index,
            });
        }
        let absolute = error.abs();
        mae += (absolute - mae) / (index + 1) as f64;
        max_abs = max_abs.max(absolute);
        errors.push(error);
    }

    let rmse = root_mean_square(errors.into_iter(), reference.len());
    if !rmse.is_finite() || !mae.is_finite() {
        return Err(MetricsError::NumericalFailure);
    }

    Ok(ErrorMetrics { rmse, mae, max_abs })
}

#[cfg(test)]
mod tests {
    use super::{ErrorMetrics, compute};
    use crate::metrics::MetricsError;

    #[test]
    fn identical_vectors_have_zero_error() {
        assert_eq!(
            compute(&[1.0, -2.0, 3.0], &[1.0, -2.0, 3.0]),
            Ok(ErrorMetrics {
                rmse: 0.0,
                mae: 0.0,
                max_abs: 0.0,
            })
        );
    }

    #[test]
    fn matches_hand_computed_values() {
        let metrics = compute(&[0.0, 0.0], &[3.0, 4.0]).unwrap();

        assert!((metrics.rmse - (12.5_f64).sqrt()).abs() < 1e-12);
        assert_eq!(metrics.mae, 3.5);
        assert_eq!(metrics.max_abs, 4.0);
    }

    #[test]
    fn rejects_empty_mismatched_and_non_finite_series() {
        assert_eq!(compute(&[], &[]), Err(MetricsError::EmptySeries));
        assert!(matches!(
            compute(&[1.0], &[1.0, 2.0]),
            Err(MetricsError::LengthMismatch { .. })
        ));
        assert!(matches!(
            compute(&[1.0], &[f64::NAN]),
            Err(MetricsError::NonFiniteValue {
                series: "estimate",
                ..
            })
        ));
    }
}
