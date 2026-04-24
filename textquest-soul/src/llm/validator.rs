//! Safety validation for local LLM observation text.

use std::{error::Error, fmt};

/// Maximum validated observation length shown to operators.
pub const MAX_OBSERVATION_CHARS: usize = 200;

const TRUNCATION_GRACE_CHARS: usize = 20;
const TRUNCATION_SUFFIX: &str = "...";
const ACTION_COMMANDS: &[&str] = &["cast", "move to", "target", "attack"];
const HALLUCINATED_MECHANICS: &[&str] = &[
    "quest marker",
    "global cooldown",
    "dodge roll",
    "combo point",
    "ultimate ability",
    "talent tree",
    "raid finder",
];

/// Reason an LLM observation was rejected before display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// The response had no displayable content.
    Empty,
    /// The response contained an observer-forbidden action command.
    ActionCommand(&'static str),
    /// The response referenced a clearly nonexistent EQ mechanic.
    HallucinatedMechanic(&'static str),
    /// The response exceeded the graceful truncation window.
    TooLong {
        /// Maximum display length.
        max_chars: usize,
        /// Actual response length.
        actual_chars: usize,
    },
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "observation is empty"),
            Self::ActionCommand(term) => {
                write!(f, "observation contains forbidden action command: {term}")
            }
            Self::HallucinatedMechanic(term) => {
                write!(f, "observation references nonexistent EQ mechanic: {term}")
            }
            Self::TooLong {
                max_chars,
                actual_chars,
            } => write!(
                f,
                "observation is too long: {actual_chars} chars exceeds {max_chars}"
            ),
        }
    }
}

impl Error for ValidationError {}

/// Validate and normalize a Gemma/local-LLM observation before operator display.
///
/// # Errors
///
/// Returns a [`ValidationError`] when the text is empty, commands the game
/// client to act, references clearly hallucinated mechanics, or is too long to
/// truncate safely.
pub fn validate_observation(text: &str) -> Result<String, ValidationError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(ValidationError::Empty);
    }

    let normalized = trimmed.to_ascii_lowercase();
    let normalized_words = normalized.split_whitespace().collect::<Vec<_>>().join(" ");

    if let Some(term) = ACTION_COMMANDS
        .iter()
        .copied()
        .find(|term| contains_action_command(&normalized_words, term))
    {
        return Err(ValidationError::ActionCommand(term));
    }

    if let Some(term) = HALLUCINATED_MECHANICS
        .iter()
        .copied()
        .find(|term| normalized_words.contains(term))
    {
        return Err(ValidationError::HallucinatedMechanic(term));
    }

    let actual_chars = trimmed.chars().count();
    if actual_chars <= MAX_OBSERVATION_CHARS {
        return Ok(trimmed.to_string());
    }

    if actual_chars <= MAX_OBSERVATION_CHARS + TRUNCATION_GRACE_CHARS {
        return Ok(truncate_observation(trimmed));
    }

    Err(ValidationError::TooLong {
        max_chars: MAX_OBSERVATION_CHARS,
        actual_chars,
    })
}

fn contains_action_command(normalized_words: &str, term: &str) -> bool {
    if term.contains(' ') {
        normalized_words.contains(term)
    } else {
        normalized_words
            .split(|c: char| !c.is_ascii_alphanumeric())
            .any(|word| word == term)
    }
}

fn truncate_observation(text: &str) -> String {
    let suffix_chars = TRUNCATION_SUFFIX.chars().count();
    let target_chars = MAX_OBSERVATION_CHARS.saturating_sub(suffix_chars);
    let mut cutoff_byte = text.len();

    for (chars_seen, (idx, _)) in text.char_indices().enumerate() {
        if chars_seen == target_chars {
            cutoff_byte = idx;
            break;
        }
    }

    let candidate = &text[..cutoff_byte];
    let word_boundary = candidate
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace())
        .map(|(idx, _)| idx);

    let prefix = word_boundary
        .filter(|idx| *idx >= target_chars.saturating_sub(40))
        .map_or(candidate, |idx| &candidate[..idx])
        .trim_end();

    format!("{prefix}{TRUNCATION_SUFFIX}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_observation_accepts_and_trims_safe_text() {
        let result = validate_observation("  Watching the camp settle down.  ")
            .expect("safe observation should validate");

        assert_eq!(result, "Watching the camp settle down.");
    }

    #[test]
    fn validate_observation_rejects_empty_text() {
        let result = validate_observation(" \n\t ");

        assert_eq!(result, Err(ValidationError::Empty));
    }

    #[test]
    fn validate_observation_rejects_action_commands() {
        for (text, term) in [
            ("cast clarity on the group", "cast"),
            ("Move to the bridge", "move to"),
            ("target the named", "target"),
            ("attack the add", "attack"),
        ] {
            let result = validate_observation(text);

            assert_eq!(result, Err(ValidationError::ActionCommand(term)));
        }
    }

    #[test]
    fn validate_observation_rejects_hallucinated_eq_mechanics() {
        let result = validate_observation("A quest marker is glowing over the camp.");

        assert_eq!(
            result,
            Err(ValidationError::HallucinatedMechanic("quest marker"))
        );
    }

    #[test]
    fn validate_observation_rejects_far_over_limit_text() {
        let text = "x".repeat(MAX_OBSERVATION_CHARS + TRUNCATION_GRACE_CHARS + 1);
        let result = validate_observation(&text);

        assert_eq!(
            result,
            Err(ValidationError::TooLong {
                max_chars: MAX_OBSERVATION_CHARS,
                actual_chars: MAX_OBSERVATION_CHARS + TRUNCATION_GRACE_CHARS + 1,
            })
        );
    }

    #[test]
    fn validate_observation_truncates_slightly_over_limit_text() {
        let text = "calm ".repeat(41);
        let result = validate_observation(&text).expect("slightly long text should truncate");

        assert!(result.chars().count() <= MAX_OBSERVATION_CHARS);
        assert!(result.ends_with(TRUNCATION_SUFFIX));
        assert_ne!(result, text.trim());
    }
}
