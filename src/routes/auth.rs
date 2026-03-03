use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use axum::{Extension, Json, extract::State, http::StatusCode};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use tracing::info;
use uuid::Uuid;

use crate::{
    api_error::ApiError, app_state::AppState, auth_middleware::AuthenticatedUser,
    security::issue_jwt,
};

#[derive(Deserialize)]
pub struct SignupRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub user_id: Uuid,
    pub email: String,
}

#[derive(Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(FromRow)]
struct UserRecord {
    id: Uuid,
    email: String,
    password_hash: String,
}

#[derive(Serialize)]
pub struct MeResponse {
    pub user_id: Uuid,
    pub email: String,
}

pub async fn signup(
    State(state): State<AppState>,
    Json(payload): Json<SignupRequest>,
) -> Result<(StatusCode, Json<AuthResponse>), ApiError> {
    let email = payload.email.trim().to_lowercase();
    validate_auth_input(&email, &payload.password)?;

    let user_id = Uuid::new_v4();
    let password_hash = hash_password(&payload.password)?;

    let inserted = sqlx::query(
        "INSERT INTO users (id, email, password_hash) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .bind(&email)
    .bind(password_hash)
    .execute(&state.db_pool)
    .await
    .map_err(crate::api_error::internal_error)?;

    if inserted.rows_affected() == 0 {
        return Err((StatusCode::CONFLICT, "email already exists".to_string()));
    }

    let auth = issue_auth_tokens(&state, user_id, email.clone()).await?;
    info!(user_id = %user_id, email = %email, "signup completed");

    Ok((StatusCode::CREATED, Json(auth)))
}

pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<SignupRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let email = payload.email.trim().to_lowercase();

    let user = sqlx::query_as::<_, UserRecord>(
        "SELECT id, email, password_hash FROM users WHERE email = $1",
    )
    .bind(&email)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(crate::api_error::internal_error)?
    .ok_or((StatusCode::UNAUTHORIZED, "invalid credentials".to_string()))?;

    verify_password(&payload.password, &user.password_hash)?;

    let auth = issue_auth_tokens(&state, user.id, user.email.clone()).await?;
    info!(user_id = %user.id, email = %user.email, "login completed");

    Ok(Json(auth))
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

    let token_hash = hash_refresh_token(payload.refresh_token.trim());
    let now = Utc::now();

    let row = sqlx::query_as::<_, RefreshTokenRecord>(
        "SELECT rt.id, rt.user_id, u.email
         FROM refresh_tokens rt
         JOIN users u ON u.id = rt.user_id
         WHERE rt.token_hash = $1
           AND rt.revoked_at IS NULL
           AND rt.expires_at > $2",
    )
    .bind(&token_hash)
    .bind(now)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(crate::api_error::internal_error)?
    .ok_or((
        StatusCode::UNAUTHORIZED,
        "invalid refresh token".to_string(),
    ))?;

    sqlx::query("UPDATE refresh_tokens SET revoked_at = NOW() WHERE id = $1")
        .bind(row.id)
        .execute(&state.db_pool)
        .await
        .map_err(crate::api_error::internal_error)?;

    let auth = issue_auth_tokens(&state, row.user_id, row.email).await?;
    Ok(Json(auth))
}

pub async fn me(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
) -> Result<Json<MeResponse>, ApiError> {
    let user =
        sqlx::query_as::<_, UserRecord>("SELECT id, email, password_hash FROM users WHERE id = $1")
            .bind(user.user_id)
            .fetch_optional(&state.db_pool)
            .await
            .map_err(crate::api_error::internal_error)?
            .ok_or((StatusCode::UNAUTHORIZED, "user not found".to_string()))?;

    Ok(Json(MeResponse {
        user_id: user.id,
        email: user.email,
    }))
}

fn validate_auth_input(email: &str, password: &str) -> Result<(), ApiError> {
    if email.is_empty() || !email.contains('@') {
        return Err((
            StatusCode::BAD_REQUEST,
            "valid email is required".to_string(),
        ));
    }

    if password.len() < 8 {
        return Err((
            StatusCode::BAD_REQUEST,
            "password must be at least 8 chars".to_string(),
        ));
    }

    Ok(())
}

fn hash_password(password: &str) -> Result<String, ApiError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to hash password".to_string(),
            )
        })
}

fn verify_password(password: &str, stored_hash: &str) -> Result<(), ApiError> {
    let parsed_hash = PasswordHash::new(stored_hash)
        .map_err(|_| (StatusCode::UNAUTHORIZED, "invalid credentials".to_string()))?;

    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .map_err(|_| (StatusCode::UNAUTHORIZED, "invalid credentials".to_string()))
}

#[derive(FromRow)]
struct RefreshTokenRecord {
    id: Uuid,
    user_id: Uuid,
    email: String,
}

async fn issue_auth_tokens(
    state: &AppState,
    user_id: Uuid,
    email: String,
) -> Result<AuthResponse, ApiError> {
    let access_token = issue_jwt(user_id, &state.jwt_secret)?;
    let refresh_token = generate_refresh_token();
    let refresh_hash = hash_refresh_token(&refresh_token);
    let expires_at = Utc::now() + Duration::days(30);

    sqlx::query(
        "INSERT INTO refresh_tokens (id, user_id, token_hash, expires_at)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(Uuid::new_v4())
    .bind(user_id)
    .bind(refresh_hash)
    .bind(expires_at)
    .execute(&state.db_pool)
    .await
    .map_err(crate::api_error::internal_error)?;

    Ok(AuthResponse {
        access_token,
        refresh_token,
        user_id,
        email,
    })
}

fn generate_refresh_token() -> String {
    format!("{}.{}", Uuid::new_v4(), Uuid::new_v4())
}

fn hash_refresh_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let digest = hasher.finalize();
    hex::encode(digest)
}
