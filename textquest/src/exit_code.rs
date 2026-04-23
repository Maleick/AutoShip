//! Canonical exit codes for TextQuest binaries.
//!
//! Standardized codes used by all TextQuest CLI entry points.
//! Inspired by BSD sysexits.h where applicable.

use std::fmt;

/// Canonical exit codes for TextQuest processes.
///
/// # Codes
/// - `0` — Success: all work completed normally.
/// - `1` — RuntimeFailure: login failures, scenario errors, logout hangs.
/// - `2` — ConfigInvalid: bad profile, missing accounts, validation errors.
/// - `3` — Interrupted: Ctrl-C / SIGINT, duration exceeded, watchdog kill.
/// - `64` — Usage: incorrect CLI invocation (mirrors BSD sysexits EX_USAGE).
/// - `70` — Software: internal assertion or unexpected panic path (EX_SOFTWARE).
/// - `73` — CantCreate: could not create a required resource (EX_CANTCREAT).
/// - `130` — Sigint: conventional shell exit code for SIGINT (128 + 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ExitCode {
    /// All iterations completed successfully.
    Success = 0,
    /// Runtime failures: login errors, scenario timeouts, logout hangs.
    RuntimeFailure = 1,
    /// Configuration or validation errors: bad profile, missing accounts.
    ConfigInvalid = 2,
    /// Signal or timeout: Ctrl-C, duration exceeded.
    Interrupted = 3,
    /// Incorrect CLI usage (BSD EX_USAGE).
    Usage = 64,
    /// Internal software error / unexpected panic path (BSD EX_SOFTWARE).
    Software = 70,
    /// Could not create a required resource (BSD EX_CANTCREAT).
    CantCreate = 73,
    /// Terminated by SIGINT (conventional shell: 128 + 2).
    Sigint = 130,
}

impl ExitCode {
    /// Return the numeric exit code value.
    pub fn code(self) -> u8 {
        self as u8
    }

    /// Return a short human-readable reason string.
    pub fn reason(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::RuntimeFailure => "runtime failure",
            Self::ConfigInvalid => "config invalid",
            Self::Interrupted => "interrupted",
            Self::Usage => "usage error",
            Self::Software => "internal software error",
            Self::CantCreate => "cannot create resource",
            Self::Sigint => "terminated by SIGINT",
        }
    }

    /// Log a structured tracing event and exit the process with this code.
    ///
    /// Callers should flush all output before invoking this.
    pub fn exit(self) -> ! {
        tracing::info!(
            exit_code = self.code(),
            reason = self.reason(),
            "process exiting"
        );
        std::process::exit(self.code() as i32)
    }

    /// Print an operator-facing error summary to stderr, then exit.
    ///
    /// `summary` should describe what went wrong in plain language.
    /// `failures` is a list of individual failure strings (may be empty).
    /// `recommendations` is a list of actionable advice strings (may be empty).
    pub fn exit_with_summary(
        self,
        summary: &str,
        failures: &[String],
        recommendations: &[String],
    ) -> ! {
        if self != Self::Success {
            eprintln!();
            eprintln!("=== TEXTQUEST EXITING: {} ===", summary.to_uppercase());
            eprintln!("Exit code: {} ({})", self.code(), self.reason());

            if !failures.is_empty() {
                eprintln!();
                eprintln!("Failures ({}):", failures.len());
                for (i, f) in failures.iter().enumerate() {
                    eprintln!("  {}. {}", i + 1, f);
                }
            }

            if !recommendations.is_empty() {
                eprintln!();
                eprintln!("Recommendations:");
                for r in recommendations {
                    eprintln!("  - {}", r);
                }
            }

            eprintln!();
        }

        tracing::info!(
            exit_code = self.code(),
            reason = self.reason(),
            summary,
            failure_count = failures.len(),
            "process exiting"
        );

        std::process::exit(self.code() as i32)
    }
}

impl fmt::Display for ExitCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.code(), self.reason())
    }
}

impl From<ExitCode> for i32 {
    fn from(code: ExitCode) -> Self {
        code.code() as i32
    }
}

#[cfg(test)]
mod tests {
    use super::ExitCode;

    #[test]
    fn success_is_zero() {
        assert_eq!(ExitCode::Success.code(), 0);
    }

    #[test]
    fn runtime_failure_is_one() {
        assert_eq!(ExitCode::RuntimeFailure.code(), 1);
    }

    #[test]
    fn config_invalid_is_two() {
        assert_eq!(ExitCode::ConfigInvalid.code(), 2);
    }

    #[test]
    fn interrupted_is_three() {
        assert_eq!(ExitCode::Interrupted.code(), 3);
    }

    #[test]
    fn usage_is_64() {
        assert_eq!(ExitCode::Usage.code(), 64);
    }

    #[test]
    fn software_is_70() {
        assert_eq!(ExitCode::Software.code(), 70);
    }

    #[test]
    fn cant_create_is_73() {
        assert_eq!(ExitCode::CantCreate.code(), 73);
    }

    #[test]
    fn sigint_is_130() {
        assert_eq!(ExitCode::Sigint.code(), 130);
    }

    #[test]
    fn display_includes_code_and_reason() {
        let s = ExitCode::RuntimeFailure.to_string();
        assert!(s.contains('1'), "expected code 1 in display: {s}");
        assert!(
            s.contains("runtime failure"),
            "expected reason in display: {s}"
        );
    }

    #[test]
    fn into_i32_conversion() {
        let code: i32 = ExitCode::ConfigInvalid.into();
        assert_eq!(code, 2);
    }

    #[test]
    fn reason_strings_are_nonempty() {
        let codes = [
            ExitCode::Success,
            ExitCode::RuntimeFailure,
            ExitCode::ConfigInvalid,
            ExitCode::Interrupted,
            ExitCode::Usage,
            ExitCode::Software,
            ExitCode::CantCreate,
            ExitCode::Sigint,
        ];
        for code in codes {
            assert!(!code.reason().is_empty(), "reason empty for {code:?}");
        }
    }

    #[test]
    fn all_codes_distinct() {
        let codes = [
            ExitCode::Success.code(),
            ExitCode::RuntimeFailure.code(),
            ExitCode::ConfigInvalid.code(),
            ExitCode::Interrupted.code(),
            ExitCode::Usage.code(),
            ExitCode::Software.code(),
            ExitCode::CantCreate.code(),
            ExitCode::Sigint.code(),
        ];
        let unique: std::collections::HashSet<u8> = codes.iter().cloned().collect();
        assert_eq!(unique.len(), codes.len(), "duplicate exit codes detected");
    }
}
