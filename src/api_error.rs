use axum::http::StatusCode;
use tracing::warn;

pub type ApiError = (StatusCode, String);

pub fn internal_error(err: sqlx::Error) -> ApiError {
    warn!("database error: {err}");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal server error".to_string(),
    )
}
