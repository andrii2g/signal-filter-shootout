//! Exponentially weighted moving average filtering.

use crate::error::{ConfigError, ConfigResult};

use super::{FilterError, OnlineFilter, validate_measurement};

/// Validated EWMA parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EwmaConfig {
    alpha: f64,
}

impl EwmaConfig {
    /// Construct an EWMA configuration with `0 < alpha <= 1`.
    pub fn new(alpha: f64) -> ConfigResult<Self> {
        if !alpha.is_finite() {
            return Err(ConfigError::NonFinite { parameter: "alpha" });
        }
        if !(0.0 < alpha && alpha <= 1.0) {
            return Err(ConfigError::InvalidValue {
                parameter: "alpha",
                requirement: "must be greater than 0 and at most 1",
            });
        }

        Ok(Self { alpha })
    }

    /// Return the smoothing weight.
    pub fn alpha(self) -> f64 {
        self.alpha
    }
}

/// Online EWMA initialized from its first measurement.
#[derive(Debug, Clone)]
pub struct EwmaFilter {
    config: EwmaConfig,
    state: Option<f64>,
}

impl EwmaFilter {
    /// Construct a fresh filter from validated parameters.
    pub fn new(config: EwmaConfig) -> Self {
        Self {
            config,
            state: None,
        }
    }
}

impl OnlineFilter for EwmaFilter {
    fn reset(&mut self) {
        self.state = None;
    }

    fn update(&mut self, measurement: f64) -> Result<f64, FilterError> {
        validate_measurement(measurement)?;

        let next = match self.state {
            None => measurement,
            Some(previous) => {
                self.config.alpha * measurement + (1.0 - self.config.alpha) * previous
            }
        };
        if !next.is_finite() {
            return Err(FilterError::NumericalFailure);
        }

        self.state = Some(next);
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::{EwmaConfig, EwmaFilter};
    use crate::{error::ConfigError, filters::OnlineFilter};

    #[test]
    fn rejects_invalid_alpha() {
        for alpha in [0.0, -0.1, 1.1] {
            assert!(matches!(
                EwmaConfig::new(alpha),
                Err(ConfigError::InvalidValue {
                    parameter: "alpha",
                    ..
                })
            ));
        }

        for alpha in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(
                EwmaConfig::new(alpha),
                Err(ConfigError::NonFinite { parameter: "alpha" })
            );
        }
    }

    #[test]
    fn follows_known_recurrence() {
        let mut filter = EwmaFilter::new(EwmaConfig::new(0.25).unwrap());
        let actual: Vec<_> = [4.0, 8.0, 0.0]
            .into_iter()
            .map(|value| filter.update(value).unwrap())
            .collect();

        assert_eq!(actual, [4.0, 5.0, 3.75]);
    }

    #[test]
    fn alpha_one_is_identity() {
        let mut filter = EwmaFilter::new(EwmaConfig::new(1.0).unwrap());

        for value in [-2.0, 0.5, 9.0] {
            assert_eq!(filter.update(value), Ok(value));
        }
    }

    #[test]
    fn reset_uses_next_measurement_as_new_initial_state() {
        let mut filter = EwmaFilter::new(EwmaConfig::new(0.5).unwrap());
        filter.update(10.0).unwrap();
        filter.update(0.0).unwrap();

        filter.reset();

        assert_eq!(filter.update(-3.0), Ok(-3.0));
    }

    #[test]
    fn constant_input_remains_constant() {
        let mut filter = EwmaFilter::new(EwmaConfig::new(0.2).unwrap());

        for _ in 0..20 {
            assert_eq!(filter.update(7.5), Ok(7.5));
        }
    }

    #[test]
    fn non_finite_measurement_does_not_change_state() {
        let mut filter = EwmaFilter::new(EwmaConfig::new(0.5).unwrap());
        filter.update(2.0).unwrap();

        assert!(filter.update(f64::NAN).is_err());
        assert_eq!(filter.update(4.0), Ok(3.0));
    }
}
