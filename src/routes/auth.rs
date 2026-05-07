use axum::{Extension, Json, extract::State, http::StatusCode};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::SaltString;
use chrono::{Duration, Utc};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use tracing::info;
use uuid::Uuid;

use crate::{
    api_error::ApiError,
    app_state::AppState,
    auth_middleware::AuthenticatedUser,
    security::issue_jwt,
};

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

const ACCESS_TOKEN_TTL_SECS: i64 = 60 * 60 * 24 * 7;
const REFRESH_TOKEN_TTL_DAYS: i64 = 30;

#[derive(FromRow)]
struct UserRecord {
    id: Uuid,
    email: String,
    password_hash: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub async fn signup(
    State(state): State<AppState>,
    Json(payload): Json<SignupRequest>,
) -> Result<(StatusCode, Json<AuthResponse>), ApiError> {
    let email = payload.email.trim().to_lowercase();
    let password = payload.password.trim();

    if email.is_empty() || password.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "email and password are required".to_string(),
        ));
    }

    let password_hash = hash_password(password)?;
    let user_id = Uuid::new_v4();

    let insert_result = sqlx::query(
        "INSERT INTO users (id, email, password_hash) VALUES ($1, $2, $3)",
    )
    .bind(user_id)
    .bind(&email)
    .bind(&password_hash)
    .execute(&state.db_pool)
    .await;

    if let Err(err) = insert_result {
        if let Some(db_err) = err.as_database_error() {
            if db_err.is_unique_violation() {
                return Err((StatusCode::CONFLICT, "email already exists".to_string()));
            }
        }
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to create user".to_string(),
        ));
    }

    let refresh_token = create_refresh_token(&state, user_id).await?;
    let (access_token, _, _) = issue_access_token(&state, user_id, Some(&email))?;

    info!(user_id = %user_id, "signup completed");

    Ok((
        StatusCode::CREATED,
        Json(AuthResponse {
            access_token,
            refresh_token,
            user_id: user_id.to_string(),
            email,
        }),
    ))
}

pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<SignupRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let email = payload.email.trim().to_lowercase();
    let password = payload.password.trim();

    if email.is_empty() || password.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "email and password are required".to_string(),
        ));
    }

    let user = sqlx::query_as::<_, UserRecord>(
        "SELECT id, email, password_hash FROM users WHERE email = $1",
    )
    .bind(&email)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "login failed".to_string()))?
    .ok_or((StatusCode::UNAUTHORIZED, "invalid credentials".to_string()))?;

    verify_password(password, &user.password_hash)?;

    let refresh_token = create_refresh_token(&state, user.id).await?;
    let (access_token, _, _) = issue_access_token(&state, user.id, Some(&user.email))?;

    info!(user_id = %user.id, "login completed");

    Ok(Json(AuthResponse {
        access_token,
        refresh_token,
        user_id: user.id.to_string(),
        email: user.email,
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

    let raw_token = payload.refresh_token.trim();
    let token_hash = hash_refresh_token(raw_token);

    let record = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT rt.user_id, u.email
         FROM refresh_tokens rt
         JOIN users u ON u.id = rt.user_id
         WHERE rt.token_hash = $1
           AND rt.revoked_at IS NULL
           AND rt.expires_at > NOW()",
    )
    .bind(&token_hash)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "refresh failed".to_string()))?
    .ok_or((StatusCode::UNAUTHORIZED, "invalid refresh token".to_string()))?;

    sqlx::query("UPDATE refresh_tokens SET revoked_at = NOW() WHERE token_hash = $1")
        .bind(&token_hash)
        .execute(&state.db_pool)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "refresh failed".to_string()))?;

    let refresh_token = create_refresh_token(&state, record.0).await?;
    let (access_token, _, _) = issue_access_token(&state, record.0, Some(&record.1))?;

    Ok(Json(AuthResponse {
        access_token,
        refresh_token,
        user_id: record.0.to_string(),
        email: record.1,
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

fn hash_password(password: &str) -> Result<String, ApiError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "password hash failed".to_string(),
            )
        })
}

fn verify_password(password: &str, hash: &str) -> Result<(), ApiError> {
    let parsed = PasswordHash::new(hash).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            "invalid credentials".to_string(),
        )
    })?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .map_err(|_| (StatusCode::UNAUTHORIZED, "invalid credentials".to_string()))
}

fn hash_refresh_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    hex::encode(digest)
}

fn generate_refresh_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

async fn create_refresh_token(state: &AppState, user_id: Uuid) -> Result<String, ApiError> {
    let token = generate_refresh_token();
    let token_hash = hash_refresh_token(&token);
    let expires_at = Utc::now() + Duration::days(REFRESH_TOKEN_TTL_DAYS);

    sqlx::query(
        "INSERT INTO refresh_tokens (id, user_id, token_hash, expires_at)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(token_hash)
    .bind(expires_at)
    .execute(&state.db_pool)
    .await
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to create refresh token".to_string(),
        )
    })?;

    Ok(token)
}

fn issue_access_token(
    state: &AppState,
    user_id: Uuid,
    email: Option<&str>,
) -> Result<(String, usize, usize), ApiError> {
    let issued_at = Utc::now().timestamp() as usize;
    let expires_at = (Utc::now() + Duration::seconds(ACCESS_TOKEN_TTL_SECS)).timestamp() as usize;
    let token = issue_jwt(user_id, email, &state.jwt_secret, issued_at, expires_at)?;
    Ok((token, issued_at, expires_at))
}
