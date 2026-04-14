//! Soul Engine configuration validator.
//!
//! Validates [`SoulConfig`] fields and returns a list of [`ConfigError`]s.
//! An empty result means the configuration is valid.

use crate::soul::config::SoulConfig;

/// A single configuration validation error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    /// The name of the field that failed validation.
    pub field_name: String,
    /// Human-readable description of the validation failure.
    pub message: String,
}

impl ConfigError {
    fn new(field_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field_name: field_name.into(),
            message: message.into(),
        }
    }
}

/// Validates [`SoulConfig`] field values and returns all errors found.
pub struct SoulConfigValidator;

impl SoulConfigValidator {
    /// Validate the given [`SoulConfig`].
    ///
    /// Returns a `Vec<ConfigError>` — empty means the configuration is valid.
    #[must_use]
    pub fn validate(config: &SoulConfig) -> Vec<ConfigError> {
        let mut errors = Vec::new();

        // max_requests_per_character: 1–60
        if config.max_requests_per_character == 0 {
            errors.push(ConfigError::new(
                "max_requests_per_character",
                "must be greater than 0",
            ));
        } else if config.max_requests_per_character > 60 {
            errors.push(ConfigError::new(
                "max_requests_per_character",
                "must be <= 60",
            ));
        }

        // max_global_requests: 1–200
        if config.max_global_requests == 0 {
            errors.push(ConfigError::new(
                "max_global_requests",
                "must be greater than 0",
            ));
        } else if config.max_global_requests > 200 {
            errors.push(ConfigError::new("max_global_requests", "must be <= 200"));
        }

        // memory_decay_days: > 0
        if config.memory_decay_days == 0 {
            errors.push(ConfigError::new(
                "memory_decay_days",
                "must be greater than 0",
            ));
        }

        // mood_decay_rate: 0.0–1.0
        if config.mood_decay_rate < 0.0 {
            errors.push(ConfigError::new("mood_decay_rate", "must be >= 0.0"));
        } else if config.mood_decay_rate > 1.0 {
            errors.push(ConfigError::new("mood_decay_rate", "must be <= 1.0"));
        }

        // llm.base_url: must be localhost/127.0.0.1 if set (SSRF prevention)
        if let Some(err_msg) = config.llm.validate_base_url() {
            errors.push(ConfigError::new("llm.base_url", err_msg));
        }

        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soul::config::SoulConfig;

    fn valid_config() -> SoulConfig {
        SoulConfig {
            max_requests_per_character: 10,
            max_global_requests: 60,
            memory_decay_days: 30,
            mood_decay_rate: 0.05,
            ..SoulConfig::default()
        }
    }

    // 1. Happy path — all fields in range
    #[test]
    fn valid_config_returns_no_errors() {
        let errors = valid_config().validate();
        assert!(errors.is_empty(), "expected no errors, got: {errors:?}");
    }

    // 2. max_requests_per_character == 0
    #[test]
    fn max_requests_per_character_zero_is_invalid() {
        let mut c = valid_config();
        c.max_requests_per_character = 0;
        let errors = c.validate();
        assert!(
            errors
                .iter()
                .any(|e| e.field_name == "max_requests_per_character"),
            "expected error for max_requests_per_character"
        );
    }

    // 3. max_requests_per_character > 60
    #[test]
    fn max_requests_per_character_over_limit_is_invalid() {
        let mut c = valid_config();
        c.max_requests_per_character = 61;
        let errors = c.validate();
        assert!(
            errors
                .iter()
                .any(|e| e.field_name == "max_requests_per_character"),
            "expected error for max_requests_per_character > 60"
        );
    }

    // 4. max_requests_per_character == 60 is valid
    #[test]
    fn max_requests_per_character_at_boundary_is_valid() {
        let mut c = valid_config();
        c.max_requests_per_character = 60;
        assert!(c.validate().is_empty());
    }

    // 5. max_global_requests == 0
    #[test]
    fn max_global_requests_zero_is_invalid() {
        let mut c = valid_config();
        c.max_global_requests = 0;
        let errors = c.validate();
        assert!(
            errors.iter().any(|e| e.field_name == "max_global_requests"),
            "expected error for max_global_requests"
        );
    }

    // 6. max_global_requests > 200
    #[test]
    fn max_global_requests_over_limit_is_invalid() {
        let mut c = valid_config();
        c.max_global_requests = 201;
        let errors = c.validate();
        assert!(
            errors.iter().any(|e| e.field_name == "max_global_requests"),
            "expected error for max_global_requests > 200"
        );
    }

    // 7. memory_decay_days == 0
    #[test]
    fn memory_decay_days_zero_is_invalid() {
        let mut c = valid_config();
        c.memory_decay_days = 0;
        let errors = c.validate();
        assert!(
            errors.iter().any(|e| e.field_name == "memory_decay_days"),
            "expected error for memory_decay_days == 0"
        );
    }

    // 8. mood_decay_rate < 0.0
    #[test]
    fn mood_decay_rate_negative_is_invalid() {
        let mut c = valid_config();
        c.mood_decay_rate = -0.1;
        let errors = c.validate();
        assert!(
            errors.iter().any(|e| e.field_name == "mood_decay_rate"),
            "expected error for mood_decay_rate < 0.0"
        );
    }

    // 9. mood_decay_rate > 1.0
    #[test]
    fn mood_decay_rate_over_one_is_invalid() {
        let mut c = valid_config();
        c.mood_decay_rate = 1.1;
        let errors = c.validate();
        assert!(
            errors.iter().any(|e| e.field_name == "mood_decay_rate"),
            "expected error for mood_decay_rate > 1.0"
        );
    }

    // 10. mood_decay_rate at boundaries (0.0 and 1.0) are valid
    #[test]
    fn mood_decay_rate_at_boundaries_is_valid() {
        let mut c = valid_config();
        c.mood_decay_rate = 0.0;
        assert!(c.validate().is_empty(), "0.0 should be valid");
        c.mood_decay_rate = 1.0;
        assert!(c.validate().is_empty(), "1.0 should be valid");
    }

    // 11. llm.base_url empty is valid
    #[test]
    fn llm_base_url_empty_is_valid() {
        let c = valid_config();
        assert!(c.llm.validate_base_url().is_none());
    }

    // 12. llm.base_url with localhost is valid
    #[test]
    fn llm_base_url_localhost_is_valid() {
        let mut c = valid_config();
        c.llm.base_url = "http://localhost:11434".to_string();
        assert!(c.llm.validate_base_url().is_none());
    }

    // 13. llm.base_url with 127.0.0.1 is valid
    #[test]
    fn llm_base_url_127_0_0_1_is_valid() {
        let mut c = valid_config();
        c.llm.base_url = "http://127.0.0.1:11434".to_string();
        assert!(c.llm.validate_base_url().is_none());
    }

    // 14. llm.base_url with IPv6 localhost (::1) is valid
    #[test]
    fn llm_base_url_ipv6_localhost_is_valid() {
        let mut c = valid_config();
        c.llm.base_url = "http://[::1]:11434".to_string();
        assert!(c.llm.validate_base_url().is_none());
    }

    // 15. llm.base_url with remote host is rejected
    #[test]
    fn llm_base_url_remote_host_is_invalid() {
        let mut c = valid_config();
        c.llm.base_url = "http://169.254.169.254:11434".to_string();
        let err = c.llm.validate_base_url();
        assert!(err.is_some());
        assert!(err.unwrap().contains("must use localhost"));
    }

    // 16. llm.base_url with attacker IP is rejected
    #[test]
    fn llm_base_url_attacker_ip_is_invalid() {
        let mut c = valid_config();
        c.llm.base_url = "http://192.168.1.1:8080".to_string();
        let err = c.llm.validate_base_url();
        assert!(err.is_some());
        assert!(err.unwrap().contains("must use localhost"));
    }

    // 17. llm.base_url without scheme is rejected
    #[test]
    fn llm_base_url_without_scheme_is_invalid() {
        let mut c = valid_config();
        c.llm.base_url = "localhost:11434".to_string();
        let err = c.llm.validate_base_url();
        assert!(err.is_some());
        assert!(err.unwrap().contains("must start with"));
    }

    // 18. llm.base_url with https is valid for localhost
    #[test]
    fn llm_base_url_https_localhost_is_valid() {
        let mut c = valid_config();
        c.llm.base_url = "https://localhost:11434".to_string();
        assert!(c.llm.validate_base_url().is_none());
    }

    // 19. llm.base_url is validated in full config validation
    #[test]
    fn full_config_rejects_invalid_base_url() {
        let mut c = valid_config();
        c.llm.base_url = "http://attacker.com:11434".to_string();
        let errors = c.validate();
        assert!(
            errors.iter().any(|e| e.field_name == "llm.base_url"),
            "expected error for invalid llm.base_url"
        );
    }

    // 20. llm.base_url with external IP is rejected in full validation
    #[test]
    fn full_config_rejects_external_ip() {
        let mut c = valid_config();
        c.llm.base_url = "http://8.8.8.8:11434".to_string();
        let errors = c.validate();
        assert!(errors.iter().any(|e| e.field_name == "llm.base_url"));
    }
}
