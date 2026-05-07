use std::time::Duration;

use axum::http::StatusCode;
use aws_sdk_s3::presigning::PresigningConfig;
use serde::Serialize;

use crate::{api_error::ApiError, app_state::AppState};

#[derive(Clone, Serialize)]
pub struct SignedUpload {
    pub upload_url: String,
    pub token: Option<String>,
}

pub async fn create_signed_upload_url(
    state: &AppState,
    path: &str,
    content_type: &str,
    expires_in_secs: i32,
) -> Result<SignedUpload, ApiError> {
    let presign = PresigningConfig::expires_in(Duration::from_secs(expires_in_secs as u64))
        .map_err(|err| {
            (
                StatusCode::BAD_GATEWAY,
                format!("presign config failed: {err}"),
            )
        })?;

    let mut request = state
        .storage_client
        .put_object()
        .bucket(&state.storage.bucket)
        .key(path);

    if !content_type.trim().is_empty() {
        request = request.content_type(content_type.trim());
    }

    let presigned = request
        .presigned(presign)
        .await
        .map_err(|err| {
            (
                StatusCode::BAD_GATEWAY,
                format!("storage upload sign failed: {err}"),
            )
        })?;

    Ok(SignedUpload {
        upload_url: presigned.uri().to_string(),
        token: None,
    })
}

pub async fn create_signed_download_url(
    state: &AppState,
    path: &str,
    expires_in_secs: i32,
) -> Result<String, ApiError> {
    let presign = PresigningConfig::expires_in(Duration::from_secs(expires_in_secs as u64))
        .map_err(|err| {
            (
                StatusCode::BAD_GATEWAY,
                format!("presign config failed: {err}"),
            )
        })?;

    let presigned = state
        .storage_client
        .get_object()
        .bucket(&state.storage.bucket)
        .key(path)
        .presigned(presign)
        .await
        .map_err(|err| {
            (
                StatusCode::BAD_GATEWAY,
                format!("storage download sign failed: {err}"),
            )
        })?;

    Ok(presigned.uri().to_string())
}
