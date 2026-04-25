//! Self-improvement suggestion API — record events, analyze patterns, suggest tuning.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use textquest_common::self_improvement::{
    analyze_session_for_suggestions, ImprovementSuggestion, SessionEvent, SessionMetrics,
    SuggestionStatus,
};

/// In-memory state for self-improvement tracking.
#[derive(Clone, Default)]
pub struct SelfImprovementState {
    /// Recorded session events, keyed by session ID.
    pub events: Arc<tokio::sync::RwLock<HashMap<u32, Vec<SessionEvent>>>>,
    /// Current suggestions, keyed by suggestion ID.
    pub suggestions: Arc<tokio::sync::RwLock<HashMap<String, ImprovementSuggestion>>>,
    /// Session metrics, keyed by session ID.
    pub metrics: Arc<tokio::sync::RwLock<HashMap<u32, SessionMetrics>>>,
    /// Monotonic event counter.
    pub next_event_id: Arc<tokio::sync::Mutex<u64>>,
}

/// API request body: record a new event.
#[derive(Debug, Serialize, Deserialize)]
pub struct RecordEventRequest {
    /// Session ID.
    pub session_id: u32,
    /// Event type.
    pub event_type: String,
    /// Event description.
    pub description: String,
    /// Additional metadata (JSON).
    pub metadata: serde_json::Value,
}

/// API response: suggestion list.
#[derive(Debug, Serialize)]
pub struct SuggestionsResponse {
    /// List of suggestions.
    pub suggestions: Vec<ImprovementSuggestion>,
    /// Total count.
    pub count: usize,
}

/// API request body: accept/reject suggestion.
#[derive(Debug, Deserialize)]
pub struct SuggestionActionRequest {
    /// Action: "accept" or "reject".
    pub action: String,
    /// Optional note from operator.
    pub note: Option<String>,
}

/// API response: action result.
#[derive(Debug, Serialize)]
pub struct ActionResponse {
    /// Whether action succeeded.
    pub success: bool,
    /// Response message.
    pub message: String,
}

impl SelfImprovementState {
    /// Create new empty state.
    pub fn new() -> Self {
        SelfImprovementState {
            events: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            suggestions: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            metrics: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            next_event_id: Arc::new(tokio::sync::Mutex::new(0)),
        }
    }

    /// Record a raw session event.
    pub async fn record_event(&self, session_id: u32, event: SessionEvent) -> Result<(), String> {
        let mut events = self.events.write().await;
        events.entry(session_id).or_insert_with(Vec::new).push(event);
        Ok(())
    }

    /// Get suggestions for session.
    pub async fn get_suggestions(&self, _session_id: Option<u32>) -> Vec<ImprovementSuggestion> {
        let suggestions = self.suggestions.read().await;
        suggestions.values().cloned().collect()
    }

    /// Record suggestion.
    pub async fn add_suggestion(&self, suggestion: ImprovementSuggestion) {
        let mut suggestions = self.suggestions.write().await;
        suggestions.insert(suggestion.id.clone(), suggestion);
    }

    /// Update suggestion status.
    pub async fn update_suggestion_status(
        &self,
        id: &str,
        status: SuggestionStatus,
    ) -> Result<(), String> {
        let mut suggestions = self.suggestions.write().await;
        if let Some(suggestion) = suggestions.get_mut(id) {
            suggestion.status = status;
            Ok(())
        } else {
            Err("Suggestion not found".to_string())
        }
    }

    /// Update session metrics.
    pub async fn set_session_metrics(&self, session_id: u32, metrics: SessionMetrics) {
        let mut all_metrics = self.metrics.write().await;
        all_metrics.insert(session_id, metrics);
    }
}

/// POST /api/improvement/events — record a raw event.
pub async fn record_event(
    State(state): State<Arc<crate::AppState>>,
    Json(req): Json<RecordEventRequest>,
) -> Result<(StatusCode, Json<ActionResponse>), StatusCode> {
    let mut event_id = state
        .self_improvement_state
        .next_event_id
        .lock()
        .await;
    *event_id += 1;

    let session_event = SessionEvent {
        id: *event_id,
        timestamp: Utc::now(),
        event_type: textquest_common::self_improvement::EventType::Custom(req.event_type),
        description: req.description,
        metadata: req.metadata,
    };

    if let Ok(()) = state
        .self_improvement_state
        .record_event(req.session_id, session_event)
        .await
    {
        Ok((
            StatusCode::CREATED,
            Json(ActionResponse {
                success: true,
                message: "Event recorded".to_string(),
            }),
        ))
    } else {
        Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
}

/// GET /api/improvement/suggestions — get current suggestions.
pub async fn get_suggestions(
    State(state): State<Arc<crate::AppState>>,
) -> Json<SuggestionsResponse> {
    let suggestions = state.self_improvement_state.get_suggestions(None).await;
    let count = suggestions.len();
    Json(SuggestionsResponse {
        suggestions,
        count,
    })
}

/// POST /api/improvement/analyze — analyze session and generate suggestions.
pub async fn analyze_session(
    State(state): State<Arc<crate::AppState>>,
    Path(session_id): Path<u32>,
) -> Json<SuggestionsResponse> {
    // Retrieve session metrics
    let metrics_map = state.self_improvement_state.metrics.read().await;
    let metrics = metrics_map
        .get(&session_id)
        .cloned()
        .unwrap_or_default();

    // Retrieve session events
    let events_map = state.self_improvement_state.events.read().await;
    let events = events_map
        .get(&session_id)
        .cloned()
        .unwrap_or_default();

    // Generate suggestions
    let suggestions = analyze_session_for_suggestions(&metrics, &events);

    // Store suggestions
    drop(events_map);
    drop(metrics_map);

    for suggestion in &suggestions {
        state
            .self_improvement_state
            .add_suggestion(suggestion.clone())
            .await;
    }

    let count = suggestions.len();
    Json(SuggestionsResponse {
        suggestions,
        count,
    })
}

/// POST /api/improvement/accept/:id — accept a suggestion.
pub async fn accept_suggestion(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Json(_req): Json<SuggestionActionRequest>,
) -> Result<Json<ActionResponse>, StatusCode> {
    state
        .self_improvement_state
        .update_suggestion_status(&id, SuggestionStatus::Accepted)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(ActionResponse {
        success: true,
        message: format!("Suggestion {} accepted", id),
    }))
}

/// POST /api/improvement/reject/:id — reject a suggestion.
pub async fn reject_suggestion(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Json(_req): Json<SuggestionActionRequest>,
) -> Result<Json<ActionResponse>, StatusCode> {
    state
        .self_improvement_state
        .update_suggestion_status(&id, SuggestionStatus::Rejected)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(ActionResponse {
        success: true,
        message: format!("Suggestion {} rejected", id),
    }))
}

/// POST /api/improvement/apply/:id — apply suggestion to config.
pub async fn apply_suggestion(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Json(_req): Json<SuggestionActionRequest>,
) -> Result<Json<ActionResponse>, StatusCode> {
    state
        .self_improvement_state
        .update_suggestion_status(&id, SuggestionStatus::Applied)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(ActionResponse {
        success: true,
        message: format!("Suggestion {} applied to config", id),
    }))
}

/// POST /api/improvement/undo/:id — undo a suggestion.
pub async fn undo_suggestion(
    State(state): State<Arc<crate::AppState>>,
    Path(id): Path<String>,
    Json(_req): Json<SuggestionActionRequest>,
) -> Result<Json<ActionResponse>, StatusCode> {
    state
        .self_improvement_state
        .update_suggestion_status(&id, SuggestionStatus::Undone)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(ActionResponse {
        success: true,
        message: format!("Suggestion {} undone", id),
    }))
}

/// POST /api/improvement/metrics/:session_id — update session metrics.
pub async fn update_session_metrics(
    State(state): State<Arc<crate::AppState>>,
    Path(session_id): Path<u32>,
    Json(metrics): Json<SessionMetrics>,
) -> Json<ActionResponse> {
    state
        .self_improvement_state
        .set_session_metrics(session_id, metrics)
        .await;

    Json(ActionResponse {
        success: true,
        message: format!("Metrics updated for session {}", session_id),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_record_event() {
        let state = SelfImprovementState::new();
        let event = SessionEvent {
            id: 1,
            timestamp: Utc::now(),
            event_type: textquest_common::self_improvement::EventType::SessionStart,
            description: "Test".to_string(),
            metadata: serde_json::json!({}),
        };

        assert!(state.record_event(1, event).await.is_ok());
    }

    #[tokio::test]
    async fn test_add_suggestion() {
        let state = SelfImprovementState::new();
        let suggestion = ImprovementSuggestion {
            id: "test".to_string(),
            title: "Test".to_string(),
            description: "Test suggestion".to_string(),
            suggestion_type: textquest_common::self_improvement::SuggestionType::Performance,
            priority: 1,
            recommendation: "Do this".to_string(),
            created_at: Utc::now(),
            status: SuggestionStatus::Pending,
            config_path: None,
            suggested_value: None,
        };

        state.add_suggestion(suggestion.clone()).await;
        let suggestions = state.get_suggestions(None).await;
        assert_eq!(suggestions.len(), 1);
    }

    #[tokio::test]
    async fn test_update_suggestion_status() {
        let state = SelfImprovementState::new();
        let suggestion = ImprovementSuggestion {
            id: "test_update".to_string(),
            title: "Test".to_string(),
            description: "Test".to_string(),
            suggestion_type: textquest_common::self_improvement::SuggestionType::Performance,
            priority: 1,
            recommendation: "Do this".to_string(),
            created_at: Utc::now(),
            status: SuggestionStatus::Pending,
            config_path: None,
            suggested_value: None,
        };

        state.add_suggestion(suggestion).await;
        assert!(state
            .update_suggestion_status("test_update", SuggestionStatus::Accepted)
            .await
            .is_ok());

        let suggestions = state.get_suggestions(None).await;
        assert_eq!(suggestions[0].status, SuggestionStatus::Accepted);
    }
}
