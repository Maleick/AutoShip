//! Sentiment scorer — keyword-based text analysis for the Soul Engine.
//!
//! Provides a simple, dependency-free sentiment scorer that returns a score in
//! the range `[-1.0, 1.0]` based on keyword matching. Positive keywords
//! contribute `+0.3` each; negative keywords contribute `-0.3` each. The
//! result is clamped to the valid range.
//!
//! The `apply_chat_sentiment` bridge wires the scorer to the social graph and
//! returns a `SoulEvent` ready for the personality engine.

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

use textquest_common::soul::SoulEvent;

use crate::social::{SocialEvent, SocialGraph};

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

/// Threshold above/below which chat is considered positive/negative.
///
/// Matches the boundary used in `personality.rs::process_event` so the social
/// graph and mood engine agree on what counts as a meaningful signal.
const SENTIMENT_THRESHOLD: f32 = 0.3;

/// Score a player chat message, apply the result to the social graph, and
/// return a `SoulEvent::PlayerChat` ready for the personality engine.
///
/// The social graph is only updated when the score exceeds `±SENTIMENT_THRESHOLD`;
/// neutral messages leave the relationship unchanged.
pub fn apply_chat_sentiment(
    text: &str,
    player_name: &str,
    bot_name: &str,
    graph: &mut SocialGraph,
) -> SoulEvent {
    let sentiment = score_sentiment(text);

    if sentiment >= SENTIMENT_THRESHOLD {
        graph.apply_event(player_name, bot_name, &SocialEvent::PositiveChat);
    } else if sentiment <= -SENTIMENT_THRESHOLD {
        graph.apply_event(player_name, bot_name, &SocialEvent::NegativeChat);
    }

    SoulEvent::PlayerChat {
        player_name: player_name.to_string(),
        sentiment,
    }
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

    // --- apply_chat_sentiment bridge tests ---

    #[test]
    fn apply_chat_sentiment_positive_updates_graph() {
        let mut graph = SocialGraph::new();
        let event = apply_chat_sentiment("awesome job everyone!", "PlayerA", "BotX", &mut graph);

        // Social graph: positive chat raises faction
        let rel = graph.get("PlayerA", "BotX").unwrap();
        assert!(rel.faction_score > 0, "positive chat should raise faction");

        // Returned event carries the sentiment score
        match event {
            SoulEvent::PlayerChat {
                player_name,
                sentiment,
            } => {
                assert_eq!(player_name, "PlayerA");
                assert!(sentiment > 0.0);
            }
            _ => panic!("expected SoulEvent::PlayerChat"),
        }
    }

    #[test]
    fn apply_chat_sentiment_negative_updates_graph() {
        let mut graph = SocialGraph::new();
        let event = apply_chat_sentiment("you trash noob!", "PlayerB", "BotY", &mut graph);

        let rel = graph.get("PlayerB", "BotY").unwrap();
        assert!(rel.faction_score < 0, "negative chat should lower faction");

        match event {
            SoulEvent::PlayerChat {
                player_name,
                sentiment,
            } => {
                assert_eq!(player_name, "PlayerB");
                assert!(sentiment < 0.0);
            }
            _ => panic!("expected SoulEvent::PlayerChat"),
        }
    }

    #[test]
    fn apply_chat_sentiment_neutral_does_not_update_graph() {
        let mut graph = SocialGraph::new();
        let _event = apply_chat_sentiment("pulling the next camp", "PlayerC", "BotZ", &mut graph);

        // Neutral message: no edge created in the graph
        assert!(
            graph.get("PlayerC", "BotZ").is_none(),
            "neutral chat must not create a relationship edge"
        );
    }

    #[test]
    fn apply_chat_sentiment_returns_player_name() {
        let mut graph = SocialGraph::new();
        let event = apply_chat_sentiment("great pull!", "Kaelthas", "Kira", &mut graph);
        match event {
            SoulEvent::PlayerChat { player_name, .. } => {
                assert_eq!(player_name, "Kaelthas");
            }
            _ => panic!("expected SoulEvent::PlayerChat"),
        }
    }

    #[test]
    fn apply_chat_sentiment_boundary_positive_at_threshold() {
        // Single "nice" → +0.3, exactly at threshold, should trigger PositiveChat
        let mut graph = SocialGraph::new();
        apply_chat_sentiment("nice", "P", "B", &mut graph);
        let rel = graph.get("P", "B").unwrap();
        assert!(rel.faction_score > 0);
    }

    #[test]
    fn apply_chat_sentiment_boundary_negative_at_threshold() {
        // Single "sucks" → -0.3, exactly at threshold, should trigger NegativeChat
        let mut graph = SocialGraph::new();
        apply_chat_sentiment("sucks", "P", "B", &mut graph);
        let rel = graph.get("P", "B").unwrap();
        assert!(rel.faction_score < 0);
    }
}
