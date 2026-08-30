//! Sliding-window median filtering.

use std::collections::VecDeque;

use crate::error::{ConfigError, ConfigResult};

use super::{FilterError, OnlineFilter, validate_measurement};

/// Validated sliding-median parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MedianConfig {
    window: usize,
}

impl MedianConfig {
    /// Construct a configuration with an odd, nonzero window length.
    pub fn new(window: usize) -> ConfigResult<Self> {
        if window == 0 || window.is_multiple_of(2) {
            return Err(ConfigError::InvalidValue {
                parameter: "window",
                requirement: "must be odd and at least 1",
            });
        }

        Ok(Self { window })
    }

    /// Return the configured maximum history length.
    pub fn window(self) -> usize {
        self.window
    }
}

/// Online median over the most recent configured window.
#[derive(Debug, Clone)]
pub struct MedianFilter {
    config: MedianConfig,
    history: VecDeque<f64>,
}

impl MedianFilter {
    /// Construct a fresh filter from validated parameters.
    pub fn new(config: MedianConfig) -> Self {
        Self {
            config,
            history: VecDeque::with_capacity(config.window),
        }
    }

    fn current_median(&self) -> f64 {
        let mut sorted: Vec<_> = self.history.iter().copied().collect();
        sorted.sort_by(f64::total_cmp);
        let middle = sorted.len() / 2;

        if sorted.len().is_multiple_of(2) {
            sorted[middle - 1] / 2.0 + sorted[middle] / 2.0
        } else {
            sorted[middle]
        }
    }
}

impl OnlineFilter for MedianFilter {
    fn reset(&mut self) {
        self.history.clear();
    }

    fn update(&mut self, measurement: f64) -> Result<f64, FilterError> {
        validate_measurement(measurement)?;

        if self.history.len() == self.config.window {
            self.history.pop_front();
        }
        self.history.push_back(measurement);

        Ok(self.current_median())
    }
}

#[cfg(test)]
mod tests {
    use super::{MedianConfig, MedianFilter};
    use crate::{error::ConfigError, filters::OnlineFilter};

    #[test]
    fn rejects_zero_and_even_windows() {
        for window in [0, 2, 4] {
            assert!(matches!(
                MedianConfig::new(window),
                Err(ConfigError::InvalidValue {
                    parameter: "window",
                    ..
                })
            ));
        }
    }

    #[test]
    fn window_one_is_identity() {
        let mut filter = MedianFilter::new(MedianConfig::new(1).unwrap());

        for value in [-4.0, 2.0, 8.0] {
            assert_eq!(filter.update(value), Ok(value));
        }
    }

    #[test]
    fn startup_prefix_uses_available_samples() {
        let mut filter = MedianFilter::new(MedianConfig::new(5).unwrap());
        let actual: Vec<_> = [4.0, 2.0, 9.0]
            .into_iter()
            .map(|value| filter.update(value).unwrap())
            .collect();

        assert_eq!(actual, [4.0, 3.0, 4.0]);
    }

    #[test]
    fn rejects_an_isolated_impulse() {
        let mut filter = MedianFilter::new(MedianConfig::new(3).unwrap());
        let actual: Vec<_> = [1.0, 1.0, 1000.0, 1.0]
            .into_iter()
            .map(|value| filter.update(value).unwrap())
            .collect();

        assert_eq!(actual, [1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn reset_clears_history() {
        let mut filter = MedianFilter::new(MedianConfig::new(3).unwrap());
        filter.update(10.0).unwrap();
        filter.update(20.0).unwrap();

        filter.reset();

        assert_eq!(filter.update(-5.0), Ok(-5.0));
    }

    #[test]
    fn handles_negative_values_and_duplicates() {
        let mut filter = MedianFilter::new(MedianConfig::new(5).unwrap());
        let actual: Vec<_> = [-3.0, -3.0, 4.0, -1.0, -1.0]
            .into_iter()
            .map(|value| filter.update(value).unwrap())
            .collect();

        assert_eq!(actual, [-3.0, -3.0, -3.0, -2.0, -1.0]);
    }

    #[test]
    fn non_finite_measurement_does_not_change_history() {
        let mut filter = MedianFilter::new(MedianConfig::new(3).unwrap());
        filter.update(1.0).unwrap();
        filter.update(3.0).unwrap();

        assert!(filter.update(f64::INFINITY).is_err());
        assert_eq!(filter.update(2.0), Ok(2.0));
    }
}
