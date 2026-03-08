use axum::{Json, extract::State, http::StatusCode};
use serde::Serialize;

use crate::app_state::AppState;

#[derive(Serialize)]
pub struct HealthResponse {
    status: &'static str,
    db: &'static str,
}

pub async fn healthz(State(state): State<AppState>) -> (StatusCode, Json<HealthResponse>) {
    let db_result = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.db_pool)
        .await;

    if db_result.is_ok() {
        (
            StatusCode::OK,
            Json(HealthResponse {
                status: "ok",
                db: "ok",
            }),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(HealthResponse {
                status: "degraded",
                db: "error",
            }),
        )
    }
}
