use axum::{
    Extension,
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{
    api_error::{ApiError, internal_error},
    app_state::AppState,
    auth_middleware::AuthenticatedUser,
    supabase_storage,
};

#[derive(Deserialize)]
pub struct UploadUrlRequest {
    kind: String,
    content_type: String,
    byte_size: i64,
}

#[derive(Serialize)]
pub struct UploadUrlResponse {
    asset_id: Uuid,
    path: String,
    upload_url: String,
}

#[derive(Serialize)]
pub struct DownloadUrlResponse {
    download_url: String,
}

#[derive(FromRow)]
struct AssetRecord {
    storage_path: String,
}

/// アセット（画像/WorldMap）の署名付きアップロードURLを生成する。DBにassetレコードを作り、Supabase Storageの署名 URLを返す。
pub async fn create_upload_url(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(payload): Json<UploadUrlRequest>,
) -> Result<Json<UploadUrlResponse>, ApiError> {
    if payload.byte_size <= 0 {
        return Err((StatusCode::BAD_REQUEST, "byte_size must be positive".to_string()));
    }

    if payload.content_type.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "content_type is required".to_string()));
    }

    if payload.kind != "image" && payload.kind != "worldmap" {
        return Err((StatusCode::BAD_REQUEST, "kind must be image or worldmap".to_string()));
    }

    let asset_id = Uuid::new_v4();
    let storage_path = format!("{}/{}/{}", user.user_id, payload.kind, asset_id);

    sqlx::query(
        "INSERT INTO assets (id, kind, storage_path, content_type, byte_size, created_by)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(asset_id)
    .bind(&payload.kind)
    .bind(&storage_path)
    .bind(payload.content_type.trim())
    .bind(payload.byte_size)
    .bind(user.user_id)
    .execute(&state.db_pool)
    .await
    .map_err(internal_error)?;

    let signed = supabase_storage::create_signed_upload_url(&state, &storage_path, 600).await?;

    Ok(Json(UploadUrlResponse {
        asset_id,
        path: storage_path,
        upload_url: signed.upload_url,
    }))
}

/// アセットの署名付きダウンロードURLを取得する。アクセス権（作成者または同じルームのメンバー）を確認。
pub async fn get_download_url(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(asset_id): Path<Uuid>,
) -> Result<Json<DownloadUrlResponse>, ApiError> {
    let asset = sqlx::query_as::<_, AssetRecord>(
                "SELECT a.storage_path
         FROM assets a
         WHERE a.id = $1
           AND (
             a.created_by = $2
             OR EXISTS (
                SELECT 1
                FROM worldmaps w
                JOIN room_members rm ON rm.room_id = w.room_id
                WHERE w.asset_id = a.id AND rm.user_id = $2
             )
             OR EXISTS (
                SELECT 1
                FROM placements p
                JOIN room_members rm ON rm.room_id = p.room_id
                WHERE p.image_asset_id = a.id AND rm.user_id = $2
             )
           )",
    )
    .bind(asset_id)
    .bind(user.user_id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(internal_error)?
    .ok_or((StatusCode::NOT_FOUND, "asset not found".to_string()))?;

    let download_url = supabase_storage::create_signed_download_url(&state, &asset.storage_path, 600).await?;

    Ok(Json(DownloadUrlResponse { download_url }))
}
