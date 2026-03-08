use axum::{Extension, Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use tracing::info;
use uuid::Uuid;

use crate::{api_error::ApiError, app_state::AppState, auth_middleware::AuthenticatedUser};

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct SignupRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub user_id: String,
    pub email: String,
}

#[derive(Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Serialize)]
pub struct MeResponse {
    pub user_id: Uuid,
    pub email: String,
}

// ---------------------------------------------------------------------------
// Supabase GoTrue response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct GoTrueUser {
    id: String,
    email: Option<String>,
}

#[derive(Deserialize)]
struct GoTrueSession {
    access_token: String,
    refresh_token: String,
    user: GoTrueUser,
}

#[derive(Deserialize)]
struct GoTrueError {
    #[serde(default)]
    msg: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub async fn signup(
    State(state): State<AppState>,
    Json(payload): Json<SignupRequest>,
) -> Result<(StatusCode, Json<AuthResponse>), ApiError> {
    let endpoint = format!("{}/auth/v1/signup", state.supabase.url);

    let res = state
        .http_client
        .post(&endpoint)
        .header("apikey", &state.supabase.anon_key)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "email": payload.email.trim().to_lowercase(),
            "password": payload.password,
        }))
        .send()
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                format!("supabase request failed: {e}"),
            )
        })?;

    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        let msg = parse_gotrue_error(&body).unwrap_or(body);
        return Err((
            StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
            msg,
        ));
    }

    let session: GoTrueSession = res
        .json()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("parse error: {e}")))?;

    info!(user_id = %session.user.id, "signup completed via supabase");

    Ok((
        StatusCode::CREATED,
        Json(AuthResponse {
            access_token: session.access_token,
            refresh_token: session.refresh_token,
            user_id: session.user.id,
            email: session.user.email.unwrap_or_default(),
        }),
    ))
}

pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<SignupRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let endpoint = format!("{}/auth/v1/token?grant_type=password", state.supabase.url);

    let res = state
        .http_client
        .post(&endpoint)
        .header("apikey", &state.supabase.anon_key)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "email": payload.email.trim().to_lowercase(),
            "password": payload.password,
        }))
        .send()
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                format!("supabase request failed: {e}"),
            )
        })?;

    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        let msg = parse_gotrue_error(&body).unwrap_or(body);
        return Err((
            StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::UNAUTHORIZED),
            msg,
        ));
    }

    let session: GoTrueSession = res
        .json()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("parse error: {e}")))?;

    info!(user_id = %session.user.id, "login completed via supabase");

    Ok(Json(AuthResponse {
        access_token: session.access_token,
        refresh_token: session.refresh_token,
        user_id: session.user.id,
        email: session.user.email.unwrap_or_default(),
    }))
}

pub async fn refresh(
    State(state): State<AppState>,
    Json(payload): Json<RefreshRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    if payload.refresh_token.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "refresh_token is required".to_string(),
        ));
    }

    let endpoint = format!(
        "{}/auth/v1/token?grant_type=refresh_token",
        state.supabase.url
    );

    let res = state
        .http_client
        .post(&endpoint)
        .header("apikey", &state.supabase.anon_key)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "refresh_token": payload.refresh_token.trim(),
        }))
        .send()
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                format!("supabase request failed: {e}"),
            )
        })?;

    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        let msg = parse_gotrue_error(&body).unwrap_or(body);
        return Err((
            StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::UNAUTHORIZED),
            msg,
        ));
    }

    let session: GoTrueSession = res
        .json()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("parse error: {e}")))?;

    Ok(Json(AuthResponse {
        access_token: session.access_token,
        refresh_token: session.refresh_token,
        user_id: session.user.id,
        email: session.user.email.unwrap_or_default(),
    }))
}

pub async fn me(
    _state: State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
) -> Result<Json<MeResponse>, ApiError> {
    Ok(Json(MeResponse {
        user_id: user.user_id,
        email: user.email.clone().unwrap_or_default(),
    }))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_gotrue_error(body: &str) -> Option<String> {
    let parsed: GoTrueError = serde_json::from_str(body).ok()?;
    parsed.msg.or(parsed.error_description)
}
