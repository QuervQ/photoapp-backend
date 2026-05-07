use axum::http::{
    HeaderMap,
    StatusCode,
};
use jsonwebtoken::{
    Algorithm,
    DecodingKey,
    Validation,
    decode,
    encode,
    EncodingKey,
    Header,
};
use serde::{
    Deserialize,
    Serialize,
};
use uuid::Uuid;

use crate::api_error::ApiError;

/// App JWT claims
#[derive(Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
    pub email: Option<String>,
    pub aud: String,
}

pub fn decode_jwt(token: &str, secret: &str) -> Result<JwtClaims, ApiError> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    validation.set_audience(&["photoapp"]);
    decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map(|data| data.claims)
    .map_err(|_| (StatusCode::UNAUTHORIZED, "invalid token".to_string()))
}

pub fn issue_jwt(
    user_id: Uuid,
    email: Option<&str>,
    secret: &str,
    issued_at: usize,
    expires_at: usize,
) -> Result<String, ApiError> {
    let claims = JwtClaims {
        sub: user_id.to_string(),
        exp: expires_at,
        iat: issued_at,
        email: email.map(|value| value.to_string()),
        aud: "photoapp".to_string(),
    };

    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "token encode failed".to_string()))
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
