//! Sentiment scorer — keyword-based text analysis for the Soul Engine.
//!
//! Provides a simple, dependency-free sentiment scorer that returns a score in
//! the range `[-1.0, 1.0]` based on keyword matching. Positive keywords
//! contribute `+0.3` each; negative keywords contribute `-0.3` each. The
//! result is clamped to the valid range.

/// Positive keywords and their contribution to the sentiment score.
const POSITIVE_KEYWORDS: &[&str] = &[
    "nice",
    "great",
    "thanks",
    "awesome",
    "good",
    "ty",
    "kk",
    "well done",
    "impressive",
];

/// Negative keywords and their contribution to the sentiment score.
const NEGATIVE_KEYWORDS: &[&str] = &[
    "sucks",
    "bad",
    "hate",
    "fail",
    "trash",
    "noob",
    "idiot",
    "die",
    "kill yourself",
];

/// Score of a single positive or negative keyword match.
const KEYWORD_WEIGHT: f32 = 0.3;

/// Score the sentiment of the given text.
///
/// Returns a value in `[-1.0, 1.0]`.  Each matched positive keyword adds
/// `+0.3` and each matched negative keyword adds `-0.3`.  The total is clamped
/// to the `[-1.0, 1.0]` range before being returned.
pub fn score_sentiment(text: &str) -> f32 {
    let lower = text.to_lowercase();
    let mut score: f32 = 0.0;

    for kw in POSITIVE_KEYWORDS {
        if lower.contains(kw) {
            score += KEYWORD_WEIGHT;
        }
    }

    for kw in NEGATIVE_KEYWORDS {
        if lower.contains(kw) {
            score -= KEYWORD_WEIGHT;
        }
    }

    score.clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_positive_single_keyword() {
        let score = score_sentiment("That was awesome!");
        assert!(
            (score - 0.3).abs() < f32::EPSILON,
            "expected 0.3, got {score}"
        );
    }

    #[test]
    fn test_negative_single_keyword() {
        let score = score_sentiment("This game sucks");
        assert!(
            (score - (-0.3)).abs() < f32::EPSILON,
            "expected -0.3, got {score}"
        );
    }

    #[test]
    fn test_neutral_text() {
        let score = score_sentiment("Hello everyone, ready to raid?");
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_multiple_positive_keywords() {
        // "great" + "thanks" + "nice" = 0.9
        let score = score_sentiment("great thanks nice");
        assert!(
            (score - 0.9).abs() < f32::EPSILON,
            "expected 0.9, got {score}"
        );
    }

    #[test]
    fn test_clamp_high() {
        // Enough positives to exceed 1.0 before clamping
        let score = score_sentiment("nice great thanks awesome good ty kk well done impressive");
        assert_eq!(score, 1.0, "score should be clamped to 1.0");
    }

    #[test]
    fn test_clamp_low() {
        // Enough negatives to go below -1.0 before clamping
        let score = score_sentiment("sucks bad hate fail trash noob idiot die kill yourself");
        assert_eq!(score, -1.0, "score should be clamped to -1.0");
    }

    #[test]
    fn test_mixed_keywords() {
        // "great" (+0.3) + "bad" (-0.3) = 0.0
        let score = score_sentiment("great but also bad");
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_case_insensitive() {
        let score = score_sentiment("AWESOME GREAT");
        assert!(
            (score - 0.6).abs() < f32::EPSILON,
            "expected 0.6, got {score}"
        );
    }

    #[test]
    fn test_multi_word_positive_keyword() {
        let score = score_sentiment("well done on that pull");
        assert!(
            (score - 0.3).abs() < f32::EPSILON,
            "expected 0.3, got {score}"
        );
    }

    #[test]
    fn test_multi_word_negative_keyword() {
        let score = score_sentiment("you should kill yourself noob");
        // "kill yourself" (-0.3) + "noob" (-0.3) = -0.6
        assert!(
            (score - (-0.6)).abs() < f32::EPSILON,
            "expected -0.6, got {score}"
        );
    }

    // --- Additional sentiment tests ---

    #[test]
    fn test_empty_string() {
        let score = score_sentiment("");
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_whitespace_only() {
        let score = score_sentiment("   \t\n  ");
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_positive_keywords_cancel_negative() {
        // "nice" (+0.3) + "bad" (-0.3) + "great" (+0.3) + "hate" (-0.3) = 0.0
        let score = score_sentiment("nice bad great hate");
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_keyword_embedded_in_word() {
        // "goodness" contains "good" → should still match
        let score = score_sentiment("goodness gracious");
        assert!(
            (score - 0.3).abs() < f32::EPSILON,
            "'goodness' contains 'good', expected 0.3, got {score}"
        );
    }

    #[test]
    fn test_ty_shorthand() {
        let score = score_sentiment("ty for the heal");
        assert!(
            (score - 0.3).abs() < f32::EPSILON,
            "expected 0.3, got {score}"
        );
    }

    #[test]
    fn test_kk_acknowledgement() {
        let score = score_sentiment("kk pulling now");
        assert!(
            (score - 0.3).abs() < f32::EPSILON,
            "expected 0.3, got {score}"
        );
    }

    #[test]
    fn test_mixed_case_negative() {
        let score = score_sentiment("You are TRASH");
        assert!(
            (score - (-0.3)).abs() < f32::EPSILON,
            "expected -0.3, got {score}"
        );
    }

    #[test]
    fn test_repeated_same_keyword_counted_once() {
        // "contains" check matches once per keyword, regardless of repetition
        let score = score_sentiment("good good good");
        assert!(
            (score - 0.3).abs() < f32::EPSILON,
            "each keyword only counted once, got {score}"
        );
    }
}
