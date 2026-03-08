use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

use crate::{api_error::ApiError, app_state::AppState};

#[derive(Debug, Deserialize)]
struct SignResponse {
    #[serde(rename = "signedURL")]
    signed_url: Option<String>,
    #[serde(rename = "signedUrl")]
    signed_url_alt: Option<String>,
    #[serde(rename = "url")]
    url: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct SignedUpload {
    pub upload_url: String,
    pub token: Option<String>,
}

pub async fn create_signed_upload_url(
    state: &AppState,
    path: &str,
    expires_in_secs: i32,
) -> Result<SignedUpload, ApiError> {
    let endpoint = format!(
        "{}/storage/v1/object/upload/sign/{}/{}",
        state.supabase.url, state.supabase.storage_bucket, path
    );

    let response = state
        .http_client
        .post(endpoint)
        .header("authorization", format!("Bearer {}", state.supabase.service_role_key))
        .header("apikey", &state.supabase.service_role_key)
        .json(&serde_json::json!({"expiresIn": expires_in_secs}))
        .send()
        .await
        .map_err(|err| {
            (
                StatusCode::BAD_GATEWAY,
                format!("supabase request failed: {err}"),
            )
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err((
            StatusCode::BAD_GATEWAY,
            format!("supabase upload sign failed: {status} {body}"),
        ));
    }

    let raw: SignResponse = response.json().await.map_err(|err| {
        (
            StatusCode::BAD_GATEWAY,
            format!("supabase response parse failed: {err}"),
        )
    })?;

    let url = raw
        .signed_url
        .or(raw.signed_url_alt)
        .or(raw.url)
        .ok_or((
            StatusCode::BAD_GATEWAY,
            "supabase signed upload url missing".to_string(),
        ))?;

    Ok(SignedUpload {
        upload_url: absolutize_storage_url(&state.supabase.url, &url),
        token: None,
    })
}

pub async fn create_signed_download_url(
    state: &AppState,
    path: &str,
    expires_in_secs: i32,
) -> Result<String, ApiError> {
    let endpoint = format!(
        "{}/storage/v1/object/sign/{}/{}",
        state.supabase.url, state.supabase.storage_bucket, path
    );

    let response = state
        .http_client
        .post(endpoint)
        .header("authorization", format!("Bearer {}", state.supabase.service_role_key))
        .header("apikey", &state.supabase.service_role_key)
        .json(&serde_json::json!({"expiresIn": expires_in_secs}))
        .send()
        .await
        .map_err(|err| {
            (
                StatusCode::BAD_GATEWAY,
                format!("supabase request failed: {err}"),
            )
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err((
            StatusCode::BAD_GATEWAY,
            format!("supabase download sign failed: {status} {body}"),
        ));
    }

    let raw: SignResponse = response.json().await.map_err(|err| {
        (
            StatusCode::BAD_GATEWAY,
            format!("supabase response parse failed: {err}"),
        )
    })?;

    let url = raw
        .signed_url
        .or(raw.signed_url_alt)
        .or(raw.url)
        .ok_or((
            StatusCode::BAD_GATEWAY,
            "supabase signed download url missing".to_string(),
        ))?;

    Ok(absolutize_storage_url(&state.supabase.url, &url))
}

fn absolutize_storage_url(base: &str, maybe_relative: &str) -> String {
    if maybe_relative.starts_with("http://") || maybe_relative.starts_with("https://") {
        return maybe_relative.to_string();
    }

    let base = base.trim_end_matches('/');
    if maybe_relative.starts_with('/') {
        format!("{}/storage/v1{}", base, maybe_relative)
    } else {
        format!("{}/storage/v1/{}", base, maybe_relative)
    }
}
