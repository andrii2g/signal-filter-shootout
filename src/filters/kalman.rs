//! Scalar Kalman filtering with a random-walk state model.

use crate::error::{ConfigError, ConfigResult};

use super::{FilterError, OnlineFilter, validate_measurement};

/// Validated scalar Kalman parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KalmanConfig {
    q: f64,
    r: f64,
    p0: f64,
}

impl KalmanConfig {
    /// Construct parameters satisfying `q >= 0`, `r > 0`, and `p0 >= 0`.
    pub fn new(q: f64, r: f64, p0: f64) -> ConfigResult<Self> {
        for (parameter, value) in [("q", q), ("r", r), ("p0", p0)] {
            if !value.is_finite() {
                return Err(ConfigError::NonFinite { parameter });
            }
        }
        if q < 0.0 {
            return Err(ConfigError::InvalidValue {
                parameter: "q",
                requirement: "must be at least 0",
            });
        }
        if r <= 0.0 {
            return Err(ConfigError::InvalidValue {
                parameter: "r",
                requirement: "must be greater than 0",
            });
        }
        if p0 < 0.0 {
            return Err(ConfigError::InvalidValue {
                parameter: "p0",
                requirement: "must be at least 0",
            });
        }
        if !(p0 + q).is_finite() {
            return Err(ConfigError::InvalidValue {
                parameter: "p0 + q",
                requirement: "must be representable as a finite value",
            });
        }

        Ok(Self { q, r, p0 })
    }

    /// Return process-noise variance.
    pub fn q(self) -> f64 {
        self.q
    }

    /// Return measurement-noise variance.
    pub fn r(self) -> f64 {
        self.r
    }

    /// Return initial estimate covariance.
    pub fn p0(self) -> f64 {
        self.p0
    }
}

/// Online scalar Kalman filter.
#[derive(Debug, Clone)]
pub struct ScalarKalmanFilter {
    config: KalmanConfig,
    estimate: Option<f64>,
    covariance: f64,
    latest_gain: Option<f64>,
}

impl ScalarKalmanFilter {
    /// Construct a fresh filter from validated parameters.
    pub fn new(config: KalmanConfig) -> Self {
        Self {
            config,
            estimate: None,
            covariance: config.p0,
            latest_gain: None,
        }
    }

    /// Return the current covariance.
    pub fn covariance(&self) -> f64 {
        self.covariance
    }

    /// Return the gain from the latest predict/update step.
    pub fn latest_gain(&self) -> Option<f64> {
        self.latest_gain
    }
}

impl OnlineFilter for ScalarKalmanFilter {
    fn reset(&mut self) {
        self.estimate = None;
        self.covariance = self.config.p0;
        self.latest_gain = None;
    }

    fn update(&mut self, measurement: f64) -> Result<f64, FilterError> {
        validate_measurement(measurement)?;

        let Some(estimate) = self.estimate else {
            self.estimate = Some(measurement);
            return Ok(measurement);
        };

        let prior_covariance = self.covariance + self.config.q;
        let denominator = prior_covariance + self.config.r;
        let gain = prior_covariance / denominator;
        let next_estimate = estimate + gain * (measurement - estimate);
        let next_covariance = (1.0 - gain) * prior_covariance;

        if !prior_covariance.is_finite()
            || !denominator.is_finite()
            || !gain.is_finite()
            || !(0.0..=1.0).contains(&gain)
            || !next_estimate.is_finite()
            || !next_covariance.is_finite()
            || next_covariance < 0.0
        {
            return Err(FilterError::NumericalFailure);
        }

        self.estimate = Some(next_estimate);
        self.covariance = next_covariance;
        self.latest_gain = Some(gain);
        Ok(next_estimate)
    }
}

#[cfg(test)]
mod tests {
    use super::{KalmanConfig, ScalarKalmanFilter};
    use crate::{error::ConfigError, filters::OnlineFilter};

    #[test]
    fn validates_q_r_and_p0() {
        for (q, r, p0, parameter) in [
            (-0.1, 1.0, 1.0, "q"),
            (0.1, 0.0, 1.0, "r"),
            (0.1, -1.0, 1.0, "r"),
            (0.1, 1.0, -1.0, "p0"),
        ] {
            assert!(matches!(
                KalmanConfig::new(q, r, p0),
                Err(ConfigError::InvalidValue {
                    parameter: actual,
                    ..
                }) if actual == parameter
            ));
        }

        for (q, r, p0, parameter) in [
            (f64::NAN, 1.0, 1.0, "q"),
            (0.1, f64::INFINITY, 1.0, "r"),
            (0.1, 1.0, f64::NEG_INFINITY, "p0"),
        ] {
            assert_eq!(
                KalmanConfig::new(q, r, p0),
                Err(ConfigError::NonFinite { parameter })
            );
        }
    }

    #[test]
    fn rejects_unrepresentable_initial_prediction() {
        assert!(matches!(
            KalmanConfig::new(f64::MAX, 1.0, f64::MAX),
            Err(ConfigError::InvalidValue {
                parameter: "p0 + q",
                ..
            })
        ));
    }

    #[test]
    fn constant_observations_stay_constant_with_finite_covariance() {
        let mut filter = ScalarKalmanFilter::new(KalmanConfig::new(0.01, 0.1, 1.0).unwrap());

        for _ in 0..100 {
            assert_eq!(filter.update(6.0), Ok(6.0));
            assert!(filter.covariance().is_finite());
            assert!(filter.covariance() >= 0.0);
        }
    }

    #[test]
    fn higher_r_reduces_response_to_measurement() {
        let mut low_r = ScalarKalmanFilter::new(KalmanConfig::new(0.0, 0.01, 1.0).unwrap());
        let mut high_r = ScalarKalmanFilter::new(KalmanConfig::new(0.0, 10.0, 1.0).unwrap());
        low_r.update(0.0).unwrap();
        high_r.update(0.0).unwrap();

        assert!(low_r.update(1.0).unwrap() > high_r.update(1.0).unwrap());
    }

    #[test]
    fn higher_q_increases_responsiveness() {
        let mut low_q = ScalarKalmanFilter::new(KalmanConfig::new(0.0, 1.0, 0.1).unwrap());
        let mut high_q = ScalarKalmanFilter::new(KalmanConfig::new(10.0, 1.0, 0.1).unwrap());
        low_q.update(0.0).unwrap();
        high_q.update(0.0).unwrap();

        assert!(high_q.update(1.0).unwrap() > low_q.update(1.0).unwrap());
    }

    #[test]
    fn gain_remains_in_unit_interval() {
        let mut filter = ScalarKalmanFilter::new(KalmanConfig::new(0.5, 2.0, 1.0).unwrap());
        filter.update(0.0).unwrap();

        for measurement in [1.0, -2.0, 4.0, 3.0] {
            filter.update(measurement).unwrap();
            assert!((0.0..=1.0).contains(&filter.latest_gain().unwrap()));
        }
    }

    #[test]
    fn reset_restores_initial_state() {
        let config = KalmanConfig::new(0.1, 0.5, 2.0).unwrap();
        let mut filter = ScalarKalmanFilter::new(config);
        filter.update(10.0).unwrap();
        filter.update(0.0).unwrap();

        filter.reset();

        assert_eq!(filter.covariance(), config.p0());
        assert_eq!(filter.latest_gain(), None);
        assert_eq!(filter.update(-4.0), Ok(-4.0));
    }

    #[test]
    fn failed_update_does_not_poison_state() {
        let mut filter = ScalarKalmanFilter::new(KalmanConfig::new(0.0, 1.0, 1.0).unwrap());
        filter.update(0.0).unwrap();

        assert!(filter.update(f64::NAN).is_err());
        assert_eq!(filter.update(1.0), Ok(0.5));
    }
}
