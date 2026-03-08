use axum::{
    extract::State,
    http::{
        HeaderMap,
        Request,
        StatusCode,
    },
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

use crate::{
    api_error::ApiError,
    app_state::AppState,
    security::{
        decode_jwt,
        extract_bearer_token,
        parse_user_id,
    },
};

#[derive(Clone, Debug)]
pub struct AuthenticatedUser {
    pub user_id: Uuid,
    pub email: Option<String>,
}

pub async fn require_auth(
    State(state): State<AppState>,
    mut req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let (user_id, email) = extract_user_from_headers(req.headers(), &state.jwt_secret, None)?;
    req.extensions_mut().insert(AuthenticatedUser { user_id, email });
    Ok(next.run(req).await)
}

pub fn extract_user_id_from_headers(
    headers: &HeaderMap,
    jwt_secret: &str,
    query_token: Option<&str>,
) -> Result<Uuid, ApiError> {
    let (user_id, _) = extract_user_from_headers(headers, jwt_secret, query_token)?;
    Ok(user_id)
}

fn extract_user_from_headers(
    headers: &HeaderMap,
    jwt_secret: &str,
    query_token: Option<&str>,
) -> Result<(Uuid, Option<String>), ApiError> {
    let token = extract_bearer_token(headers).or_else(|| query_token.map(str::to_string));
    let token = token.ok_or((
        StatusCode::UNAUTHORIZED,
        "missing bearer token".to_string(),
    ))?;
    let claims = decode_jwt(&token, jwt_secret)?;
    let user_id = parse_user_id(&claims)?;
    Ok((user_id, claims.email))
}
