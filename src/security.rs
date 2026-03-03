use axum::http::{
    HeaderMap,
    StatusCode,
};
use chrono::Utc;
use jsonwebtoken::{
    Algorithm,
    DecodingKey,
    EncodingKey,
    Header,
    Validation,
    decode,
    encode,
};
use serde::{
    Deserialize,
    Serialize,
};
use uuid::Uuid;

use crate::api_error::ApiError;

#[derive(Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
}

pub fn issue_jwt(user_id: Uuid, secret: &str) -> Result<String, ApiError> {
    let now = Utc::now().timestamp() as usize;
    let exp = now + (60 * 60 * 24 * 7);
    let claims = JwtClaims {
        sub: user_id.to_string(),
        iat: now,
        exp,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to issue token".to_string(),
        )
    })
}

pub fn decode_jwt(token: &str, secret: &str) -> Result<JwtClaims, ApiError> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map(|data| data.claims)
    .map_err(|_| (StatusCode::UNAUTHORIZED, "invalid token".to_string()))
}

pub fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    let value = headers.get("authorization")?.to_str().ok()?;
    let token = value.strip_prefix("Bearer ")?;
    Some(token.to_string())
}

pub fn parse_user_id(claims: &JwtClaims) -> Result<Uuid, ApiError> {
    Uuid::parse_str(&claims.sub).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            "invalid token subject".to_string(),
        )
    })
}
