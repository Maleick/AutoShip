//! Operator notification system for session-end suggestions.
//!
//! Tracks available suggestions from the self-improvement loop and notifies
//! the operator via web UI badge + optional Discord webhook. Never auto-applies
//! suggestions — always operator-in-the-loop.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::AppState;
use super::json_error;

/// Status of a single tuning suggestion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionStatus {
    /// New suggestion not yet reviewed by operator.
    New,
    /// Operator has reviewed but not acted on it.
    Reviewed,
    /// Operator has accepted the suggestion.
    Accepted,
    /// Operator has dismissed/rejected the suggestion.
    Dismissed,
}

/// Tuning target categories for suggestions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionCategory {
    CampAggroRadius,
    PullCadence,
    MedBreakMana,
    RetreatHp,
    CombatAbilityPriority,
    HealTriggers,
    RouteWaypoints,
    Other(String),
}

/// A single tuning suggestion from the self-improvement loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    /// Unique suggestion ID (timestamp-based).
    pub id: u64,
    /// Category of tuning target.
    pub category: SuggestionCategory,
    /// Human-readable description of the suggestion.
    pub description: String,
    /// Current value (if numeric).
    pub current_value: Option<String>,
    /// Recommended value.
    pub recommended_value: String,
    /// Confidence level (0.0-1.0) from the heuristic engine.
    pub confidence: f64,
    /// Status (new, reviewed, accepted, dismissed).
    pub status: SuggestionStatus,
    /// When the suggestion was generated (Unix timestamp).
    pub created_at: u64,
    /// When the operator last acted on it.
    pub last_updated_at: u64,
    /// Session that triggered this suggestion.
    pub session_id: Option<u32>,
}

impl Suggestion {
    /// Create a new suggestion.
    pub fn new(
        category: SuggestionCategory,
        description: String,
        current_value: Option<String>,
        recommended_value: String,
        confidence: f64,
        session_id: Option<u32>,
    ) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            id: now,
            category,
            description,
            current_value,
            recommended_value,
            confidence,
            status: SuggestionStatus::New,
            created_at: now,
            last_updated_at: now,
            session_id,
        }
    }
}

/// In-memory suggestion store.
pub struct SuggestionState {
    suggestions: RwLock<Vec<Suggestion>>,
}

impl SuggestionState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            suggestions: RwLock::new(Vec::new()),
        })
    }

    pub async fn add(&self, suggestion: Suggestion) -> Result<(), String> {
        let mut suggestions = self.suggestions.write().await;
        suggestions.push(suggestion);
        Ok(())
    }

    pub async fn get_all(&self) -> Result<Vec<Suggestion>, String> {
        Ok(self.suggestions.read().await.clone())
    }

    pub async fn get_unread_count(&self) -> Result<usize, String> {
        let suggestions = self.suggestions.read().await;
        Ok(suggestions.iter().filter(|s| s.status == SuggestionStatus::New).count())
    }

    pub async fn update_status(
        &self,
        suggestion_id: u64,
        new_status: SuggestionStatus,
    ) -> Result<Option<Suggestion>, String> {
        let mut suggestions = self.suggestions.write().await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if let Some(suggestion) = suggestions.iter_mut().find(|s| s.id == suggestion_id) {
            suggestion.status = new_status;
            suggestion.last_updated_at = now;
            Ok(Some(suggestion.clone()))
        } else {
            Ok(None)
        }
    }
}

impl Default for SuggestionState {
    fn default() -> Self {
        Self::new().as_ref().clone()
    }
}

/// Response for listing suggestions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestionsResponse {
    pub suggestions: Vec<Suggestion>,
    pub unread_count: usize,
}

/// Request to update a suggestion's status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSuggestionRequest {
    pub status: SuggestionStatus,
}

/// Response after updating a suggestion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSuggestionResponse {
    pub suggestion: Suggestion,
    pub unread_count: usize,
}

pub fn router() -> axum::Router<Arc<AppState>> {
    axum::Router::new()
        .route("/", get(list_suggestions))
        .route("/unread-count", get(get_unread_count))
        .route("/{id}", get(get_suggestion).put(update_suggestion))
        .route("/{id}/dismiss", post(dismiss_suggestion))
        .route("/{id}/accept", post(accept_suggestion))
}

/// GET /api/suggestions — list all suggestions.
pub async fn list_suggestions(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match (
        state.suggestion_state.get_all().await,
        state.suggestion_state.get_unread_count().await,
    ) {
        (Ok(suggestions), Ok(unread_count)) => (
            StatusCode::OK,
            Json(SuggestionsResponse {
                suggestions,
                unread_count,
            }),
        )
            .into_response(),
        (Err(error), _) | (_, Err(error)) => {
            tracing::error!(%error, "Failed to list suggestions");
            json_error(StatusCode::INTERNAL_SERVER_ERROR, "Failed to list suggestions")
                .into_response()
        }
    }
}

/// GET /api/suggestions/unread-count — get count of new suggestions.
pub async fn get_unread_count(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.suggestion_state.get_unread_count().await {
        Ok(count) => (
            StatusCode::OK,
            Json(serde_json::json!({ "unread_count": count })),
        )
            .into_response(),
        Err(error) => {
            tracing::error!(%error, "Failed to get unread count");
            json_error(StatusCode::INTERNAL_SERVER_ERROR, "Failed to get unread count")
                .into_response()
        }
    }
}

/// GET /api/suggestions/{id} — get a specific suggestion.
pub async fn get_suggestion(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
    let suggestions = match state.suggestion_state.get_all().await {
        Ok(s) => s,
        Err(error) => {
            tracing::error!(%error, suggestion_id = id, "Failed to fetch suggestion");
            return json_error(StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch suggestion")
                .into_response();
        }
    };

    if let Some(suggestion) = suggestions.iter().find(|s| s.id == id) {
        (StatusCode::OK, Json(suggestion.clone())).into_response()
    } else {
        json_error(StatusCode::NOT_FOUND, format!("Suggestion {id} not found")).into_response()
    }
}

/// PUT /api/suggestions/{id} — update suggestion status.
pub async fn update_suggestion(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u64>,
    Json(req): Json<UpdateSuggestionRequest>,
) -> impl IntoResponse {
    match state
        .suggestion_state
        .update_status(id, req.status.clone())
        .await
    {
        Ok(Some(suggestion)) => match state.suggestion_state.get_unread_count().await {
            Ok(unread_count) => (
                StatusCode::OK,
                Json(UpdateSuggestionResponse {
                    suggestion,
                    unread_count,
                }),
            )
                .into_response(),
            Err(error) => {
                tracing::error!(%error, "Failed to get unread count");
                json_error(StatusCode::INTERNAL_SERVER_ERROR, "Failed to get unread count")
                    .into_response()
            }
        },
        Ok(None) => json_error(StatusCode::NOT_FOUND, format!("Suggestion {id} not found"))
            .into_response(),
        Err(error) => {
            tracing::error!(%error, suggestion_id = id, "Failed to update suggestion");
            json_error(StatusCode::INTERNAL_SERVER_ERROR, "Failed to update suggestion")
                .into_response()
        }
    }
}

/// POST /api/suggestions/{id}/dismiss — mark suggestion as dismissed.
pub async fn dismiss_suggestion(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
    match state
        .suggestion_state
        .update_status(id, SuggestionStatus::Dismissed)
        .await
    {
        Ok(Some(_)) => (StatusCode::OK, Json(serde_json::json!({ "dismissed": true })))
            .into_response(),
        Ok(None) => json_error(StatusCode::NOT_FOUND, format!("Suggestion {id} not found"))
            .into_response(),
        Err(error) => {
            tracing::error!(%error, suggestion_id = id, "Failed to dismiss suggestion");
            json_error(StatusCode::INTERNAL_SERVER_ERROR, "Failed to dismiss suggestion")
                .into_response()
        }
    }
}

/// POST /api/suggestions/{id}/accept — mark suggestion as accepted.
pub async fn accept_suggestion(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
    match state
        .suggestion_state
        .update_status(id, SuggestionStatus::Accepted)
        .await
    {
        Ok(Some(_)) => (StatusCode::OK, Json(serde_json::json!({ "accepted": true })))
            .into_response(),
        Ok(None) => json_error(StatusCode::NOT_FOUND, format!("Suggestion {id} not found"))
            .into_response(),
        Err(error) => {
            tracing::error!(%error, suggestion_id = id, "Failed to accept suggestion");
            json_error(StatusCode::INTERNAL_SERVER_ERROR, "Failed to accept suggestion")
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_suggestion_state() {
        let state = SuggestionState::new();
        assert_eq!(state.get_unread_count().await.unwrap(), 0);

        let suggestion = Suggestion::new(
            SuggestionCategory::PullCadence,
            "Increase pull cadence".to_string(),
            Some("2s".to_string()),
            "3s".to_string(),
            0.85,
            None,
        );

        state.add(suggestion.clone()).await.unwrap();
        assert_eq!(state.get_unread_count().await.unwrap(), 1);

        state
            .update_status(suggestion.id, SuggestionStatus::Dismissed)
            .await
            .unwrap();
        assert_eq!(state.get_unread_count().await.unwrap(), 0);
    }
}
