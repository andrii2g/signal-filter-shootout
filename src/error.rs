//! Shared typed errors for validated domain configuration.

use thiserror::Error;

/// Errors returned when a domain configuration cannot be constructed.
///
/// Algorithm modules should validate raw values at construction time and return
/// this type (or a module-specific typed error). Application boundaries may add
/// context, but core modules must not turn configuration failures into strings.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    /// A floating-point parameter was NaN or infinite.
    #[error("configuration parameter '{parameter}' must be finite")]
    NonFinite { parameter: &'static str },

    /// A parameter did not satisfy its documented range or shape constraint.
    #[error("configuration parameter '{parameter}' {requirement}")]
    InvalidValue {
        parameter: &'static str,
        requirement: &'static str,
    },
}

/// Result alias for validated domain configuration constructors.
pub type ConfigResult<T> = Result<T, ConfigError>;

#[cfg(test)]
mod tests {
    use super::ConfigError;

    #[test]
    fn non_finite_error_names_the_parameter() {
        let error = ConfigError::NonFinite { parameter: "alpha" };

        assert_eq!(
            error.to_string(),
            "configuration parameter 'alpha' must be finite"
        );
    }

    #[test]
    fn invalid_value_error_explains_the_requirement() {
        let error = ConfigError::InvalidValue {
            parameter: "window",
            requirement: "must be odd and at least 1",
        };

        assert_eq!(
            error.to_string(),
            "configuration parameter 'window' must be odd and at least 1"
        );
    }
}
