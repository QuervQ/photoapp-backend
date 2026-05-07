use axum::http::{
    HeaderMap,
    StatusCode,
};
use jsonwebtoken::{
    Algorithm,
    DecodingKey,
    Validation,
    decode,
};
use serde::{
    Deserialize,
    Serialize,
};
use uuid::Uuid;

use crate::api_error::ApiError;

/// Supabase Auth JWT claims
#[derive(Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: String,
    pub exp: Option<usize>,
    pub iat: Option<usize>,
    pub email: Option<String>,
    pub role: Option<String>,
    pub aud: Option<String>,
}

/// JWTトークンをHS256で検証・デコードし、クレームを返す。aud="authenticated"を検証。
pub fn decode_jwt(token: &str, secret: &str) -> Result<JwtClaims, ApiError> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    // Supabase sets aud to "authenticated"
    validation.set_audience(&["authenticated"]);
    decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map(|data| data.claims)
    .map_err(|_| (StatusCode::UNAUTHORIZED, "invalid token".to_string()))
}

/// Authorizationヘッダーから"Bearer "プレフィックスを除いてトークン文字列を抽出する。
pub fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    let value = headers.get("authorization")?.to_str().ok()?;
    let token = value.strip_prefix("Bearer ")?;
    Some(token.to_string())
}

/// JWTのsubクレームをUUIDとしてパースし、ユーザーIDを返す。
pub fn parse_user_id(claims: &JwtClaims) -> Result<Uuid, ApiError> {
    Uuid::parse_str(&claims.sub).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            "invalid token subject".to_string(),
        )
    })
}
