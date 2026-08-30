//! Deterministic sequential logarithmic Q/R grid search.

use crate::{
    error::{ConfigError, ConfigResult},
    filters::{
        FilterError, OnlineFilter,
        kalman::{KalmanConfig, ScalarKalmanFilter},
    },
    metrics::{MetricsError, error},
};
use thiserror::Error;

/// Validated candidate ranges and output count.
#[derive(Debug, Clone, PartialEq)]
pub struct GridSearchConfig {
    q_min_exp: i32,
    q_max_exp: i32,
    r_min_exp: i32,
    r_max_exp: i32,
    multipliers: Vec<f64>,
    top: usize,
}

impl GridSearchConfig {
    pub fn new(
        q_min_exp: i32,
        q_max_exp: i32,
        r_min_exp: i32,
        r_max_exp: i32,
        multipliers: Vec<f64>,
        top: usize,
    ) -> ConfigResult<Self> {
        validate_exponents("q", q_min_exp, q_max_exp)?;
        validate_exponents("r", r_min_exp, r_max_exp)?;
        if multipliers.is_empty() {
            return Err(ConfigError::InvalidValue {
                parameter: "grid_multipliers",
                requirement: "must contain at least one value",
            });
        }
        for multiplier in &multipliers {
            if !multiplier.is_finite() {
                return Err(ConfigError::NonFinite {
                    parameter: "grid_multipliers",
                });
            }
            if *multiplier <= 0.0 {
                return Err(ConfigError::InvalidValue {
                    parameter: "grid_multipliers",
                    requirement: "must contain only values greater than 0",
                });
            }
        }
        if top == 0 {
            return Err(ConfigError::InvalidValue {
                parameter: "top",
                requirement: "must be greater than 0",
            });
        }

        Ok(Self {
            q_min_exp,
            q_max_exp,
            r_min_exp,
            r_max_exp,
            multipliers,
            top,
        })
    }

    pub fn top(&self) -> usize {
        self.top
    }

    fn q_values(&self) -> Vec<f64> {
        generate_log_candidates(self.q_min_exp, self.q_max_exp, &self.multipliers)
            .expect("validated grid configuration")
    }

    fn r_values(&self) -> Vec<f64> {
        generate_log_candidates(self.r_min_exp, self.r_max_exp, &self.multipliers)
            .expect("validated grid configuration")
    }
}

impl Default for GridSearchConfig {
    fn default() -> Self {
        Self::new(-8, -1, -6, 1, vec![1.0, 3.0], 5).expect("valid default grid configuration")
    }
}

/// One evaluated Q/R pair.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KalmanCandidate {
    pub q: f64,
    pub r: f64,
    pub rmse: f64,
}

/// Fully ranked deterministic grid-search result.
#[derive(Debug, Clone, PartialEq)]
pub struct GridSearchResult {
    pub best: KalmanCandidate,
    pub ranked: Vec<KalmanCandidate>,
}

impl GridSearchResult {
    pub fn top(&self, count: usize) -> &[KalmanCandidate] {
        &self.ranked[..count.min(self.ranked.len())]
    }
}

/// Failures while evaluating a grid.
#[derive(Debug, Error)]
pub enum GridSearchError {
    #[error(transparent)]
    Configuration(#[from] ConfigError),

    #[error(transparent)]
    Filter(#[from] FilterError),

    #[error(transparent)]
    Metrics(#[from] MetricsError),
}

/// Generate sorted, exactly deduplicated multiplier-by-decade candidates.
pub fn generate_log_candidates(
    min_exp: i32,
    max_exp: i32,
    multipliers: &[f64],
) -> ConfigResult<Vec<f64>> {
    validate_exponents("candidate", min_exp, max_exp)?;
    if multipliers.is_empty() {
        return Err(ConfigError::InvalidValue {
            parameter: "grid_multipliers",
            requirement: "must contain at least one value",
        });
    }

    let mut values = Vec::new();
    for exponent in min_exp..=max_exp {
        let decade = 10_f64.powi(exponent);
        for &multiplier in multipliers {
            if !multiplier.is_finite() {
                return Err(ConfigError::NonFinite {
                    parameter: "grid_multipliers",
                });
            }
            if multiplier <= 0.0 {
                return Err(ConfigError::InvalidValue {
                    parameter: "grid_multipliers",
                    requirement: "must contain only values greater than 0",
                });
            }
            let value = multiplier * decade;
            if !value.is_finite() || value <= 0.0 {
                return Err(ConfigError::InvalidValue {
                    parameter: "grid_candidates",
                    requirement: "must be finite and greater than 0",
                });
            }
            values.push(value);
        }
    }

    values.sort_by(f64::total_cmp);
    values.dedup_by(|left, right| left.to_bits() == right.to_bits());
    Ok(values)
}

/// Evaluate every candidate sequentially and rank by RMSE, Q, then R.
pub fn search(
    measurements: &[f64],
    reference: &[f64],
    config: &GridSearchConfig,
    p0: f64,
) -> Result<GridSearchResult, GridSearchError> {
    error::compute(reference, measurements)?;

    let mut ranked = Vec::new();
    for q in config.q_values() {
        for r in config.r_values() {
            let kalman_config = KalmanConfig::new(q, r, p0)?;
            let mut filter = ScalarKalmanFilter::new(kalman_config);
            let estimates = measurements
                .iter()
                .map(|&measurement| filter.update(measurement))
                .collect::<Result<Vec<_>, _>>()?;
            let rmse = error::compute(reference, &estimates)?.rmse;
            ranked.push(KalmanCandidate { q, r, rmse });
        }
    }

    ranked.sort_by(compare_candidates);
    let best = ranked[0];
    Ok(GridSearchResult { best, ranked })
}

fn compare_candidates(left: &KalmanCandidate, right: &KalmanCandidate) -> std::cmp::Ordering {
    match (left.rmse.is_finite(), right.rmse.is_finite()) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => left
            .rmse
            .total_cmp(&right.rmse)
            .then_with(|| left.q.total_cmp(&right.q))
            .then_with(|| left.r.total_cmp(&right.r)),
    }
}

fn validate_exponents(parameter: &'static str, min: i32, max: i32) -> ConfigResult<()> {
    if min > max {
        return Err(ConfigError::InvalidValue {
            parameter,
            requirement: "minimum exponent must not exceed maximum exponent",
        });
    }
    if !(-300..=300).contains(&min) || !(-300..=300).contains(&max) {
        return Err(ConfigError::InvalidValue {
            parameter,
            requirement: "exponents must be between -300 and 300",
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{GridSearchConfig, generate_log_candidates, search};
    use crate::{
        filters::{
            OnlineFilter,
            kalman::{KalmanConfig, ScalarKalmanFilter},
        },
        metrics::error,
    };

    #[test]
    fn default_candidate_generation_is_sorted_and_deterministic() {
        let first = generate_log_candidates(-8, -1, &[1.0, 3.0]).unwrap();
        let second = generate_log_candidates(-8, -1, &[1.0, 3.0]).unwrap();

        assert_eq!(first, second);
        assert_eq!(first.len(), 16);
        assert!(first.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn candidates_are_exactly_deduplicated() {
        assert_eq!(
            generate_log_candidates(-1, 0, &[1.0, 1.0]).unwrap(),
            [0.1, 1.0]
        );
    }

    #[test]
    fn tie_breaks_by_lower_q_then_lower_r() {
        let config = GridSearchConfig::new(-2, -1, -2, -1, vec![1.0], 3).unwrap();
        let result = search(&[2.0; 8], &[2.0; 8], &config, 1.0).unwrap();

        assert_eq!(result.best.q, 0.01);
        assert_eq!(result.best.r, 0.01);
        assert_eq!(result.top(3).len(), 3);
    }

    #[test]
    fn tuned_candidate_beats_deliberately_poor_baseline() {
        let reference: Vec<_> = (0..40).map(|index| index as f64 * 0.25).collect();
        let measurements: Vec<_> = reference
            .iter()
            .enumerate()
            .map(|(index, value)| value + if index % 2 == 0 { 0.2 } else { -0.2 })
            .collect();
        let config = GridSearchConfig::new(-2, 1, -2, 1, vec![1.0, 3.0], 5).unwrap();
        let result = search(&measurements, &reference, &config, 1.0).unwrap();

        let mut poor = ScalarKalmanFilter::new(KalmanConfig::new(0.0, 1_000_000.0, 1.0).unwrap());
        let poor_values = measurements
            .iter()
            .map(|&value| poor.update(value).unwrap())
            .collect::<Vec<_>>();
        let poor_rmse = error::compute(&reference, &poor_values).unwrap().rmse;

        assert!(result.best.rmse < poor_rmse);
    }

    #[test]
    fn validates_grid_configuration() {
        assert!(GridSearchConfig::new(1, -1, -2, 1, vec![1.0], 5).is_err());
        assert!(GridSearchConfig::new(-2, 1, -2, 1, vec![], 5).is_err());
        assert!(GridSearchConfig::new(-2, 1, -2, 1, vec![f64::NAN], 5).is_err());
        assert!(GridSearchConfig::new(-2, 1, -2, 1, vec![1.0], 0).is_err());
    }
}
