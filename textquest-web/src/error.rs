//! Shared HTTP error helpers for API handlers.

use axum::{Json, http::StatusCode, response::{IntoResponse, Response}};
use textquest_common::api_types::ErrorResponse;

/// Build a JSON error response.
pub fn json_error(status: StatusCode, message: impl Into<String>) -> Response {
    json_error_pair(status, message).into_response()
}

/// Build a JSON error response as a tuple, for handlers that return
/// `(StatusCode, Json<ErrorResponse>)` directly instead of `Response`.
pub fn json_error_pair(
    status: StatusCode,
    message: impl Into<String>,
) -> (StatusCode, Json<ErrorResponse>) {
    (status, Json(ErrorResponse::new(message)))
}
