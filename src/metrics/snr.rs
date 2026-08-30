//! SNR and SNR-improvement metrics with explicit special values.

use super::{MetricsError, root_mean_square, validate_pair};

/// A signal-to-noise ratio in decibels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SnrValue {
    Finite(f64),
    Infinite,
    Undefined,
}

/// Difference between output and input SNR.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SnrImprovement {
    Finite(f64),
    PositiveInfinity,
    NegativeInfinity,
    Undefined,
}

/// Compute SNR relative to a known clean reference.
pub fn compute(reference: &[f64], estimate: &[f64]) -> Result<SnrValue, MetricsError> {
    validate_pair(reference, estimate)?;

    let signal_rms = root_mean_square(reference.iter().copied(), reference.len());
    let mut errors = Vec::with_capacity(reference.len());
    for (index, (&reference, &estimate)) in reference.iter().zip(estimate).enumerate() {
        let error = estimate - reference;
        if !error.is_finite() {
            return Err(MetricsError::NonFiniteValue {
                series: "error",
                index,
            });
        }
        errors.push(error);
    }
    let error_rms = root_mean_square(errors.into_iter(), reference.len());

    if signal_rms == 0.0 {
        return Ok(SnrValue::Undefined);
    }
    if error_rms == 0.0 {
        return Ok(SnrValue::Infinite);
    }

    let decibels = 20.0 * (signal_rms.log10() - error_rms.log10());
    if decibels.is_finite() {
        Ok(SnrValue::Finite(decibels))
    } else {
        Err(MetricsError::NumericalFailure)
    }
}

/// Compute output-SNR minus input-SNR with explicit infinity handling.
pub fn improvement(input: SnrValue, output: SnrValue) -> SnrImprovement {
    match (input, output) {
        (SnrValue::Undefined, _) | (_, SnrValue::Undefined) => SnrImprovement::Undefined,
        (SnrValue::Finite(input), SnrValue::Finite(output)) => {
            SnrImprovement::Finite(output - input)
        }
        (SnrValue::Finite(_), SnrValue::Infinite) => SnrImprovement::PositiveInfinity,
        (SnrValue::Infinite, SnrValue::Finite(_)) => SnrImprovement::NegativeInfinity,
        (SnrValue::Infinite, SnrValue::Infinite) => SnrImprovement::Finite(0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::{SnrImprovement, SnrValue, compute, improvement};

    #[test]
    fn known_power_ratio_is_ten_decibels() {
        let snr = compute(&[1.0, 1.0], &[1.0 + 10_f64.sqrt().recip(); 2]).unwrap();

        let SnrValue::Finite(value) = snr else {
            panic!("expected finite SNR");
        };
        assert!((value - 10.0).abs() < 1e-12);
    }

    #[test]
    fn perfect_estimate_has_infinite_snr() {
        assert_eq!(compute(&[1.0], &[1.0]), Ok(SnrValue::Infinite));
    }

    #[test]
    fn zero_signal_is_undefined() {
        assert_eq!(compute(&[0.0], &[1.0]), Ok(SnrValue::Undefined));
        assert_eq!(compute(&[0.0], &[0.0]), Ok(SnrValue::Undefined));
    }

    #[test]
    fn improvement_handles_finite_and_special_values() {
        assert_eq!(
            improvement(SnrValue::Finite(2.0), SnrValue::Finite(5.5)),
            SnrImprovement::Finite(3.5)
        );
        assert_eq!(
            improvement(SnrValue::Finite(2.0), SnrValue::Infinite),
            SnrImprovement::PositiveInfinity
        );
        assert_eq!(
            improvement(SnrValue::Undefined, SnrValue::Finite(2.0)),
            SnrImprovement::Undefined
        );
    }
}
