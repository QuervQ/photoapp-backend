use axum::{
    Extension,
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tracing::info;
use uuid::Uuid;

use crate::{
    api_error::{ApiError, internal_error},
    app_state::AppState,
    auth_middleware::AuthenticatedUser,
    supabase_storage,
};

use super::rooms::ensure_room_member;

#[derive(Deserialize)]
pub struct SetWorldmapRequest {
    asset_id: Uuid,
}

#[derive(Serialize)]
pub struct WorldmapResponse {
    version: i32,
    asset_id: Uuid,
    download_url: String,
    created_at: DateTime<Utc>,
}

#[derive(Serialize)]
struct WorldmapUpdatedEvent {
    r#type: &'static str,
    room_id: Uuid,
    version: i32,
}

#[derive(FromRow)]
struct WorldmapRecord {
    version: i32,
    asset_id: Uuid,
    created_at: DateTime<Utc>,
    storage_path: String,
}

pub async fn set_worldmap(
    State(state): State<AppState>,
    Path(room_id): Path<Uuid>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(payload): Json<SetWorldmapRequest>,
) -> Result<(StatusCode, Json<WorldmapResponse>), ApiError> {
    ensure_room_member(&state.db_pool, room_id, user.user_id).await?;

    let asset_path = sqlx::query_scalar::<_, String>(
        "SELECT storage_path FROM assets WHERE id = $1 AND kind = 'worldmap'",
    )
    .bind(payload.asset_id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(internal_error)?
    .ok_or((StatusCode::NOT_FOUND, "worldmap asset not found".to_string()))?;

    let next_version = sqlx::query_scalar::<_, i32>(
        "SELECT COALESCE(MAX(version), 0) + 1 FROM worldmaps WHERE room_id = $1",
    )
    .bind(room_id)
    .fetch_one(&state.db_pool)
    .await
    .map_err(internal_error)?;

    sqlx::query(
        "INSERT INTO worldmaps (room_id, version, asset_id, created_by)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(room_id)
    .bind(next_version)
    .bind(payload.asset_id)
    .bind(user.user_id)
    .execute(&state.db_pool)
    .await
    .map_err(internal_error)?;

    let download_url = supabase_storage::create_signed_download_url(&state, &asset_path, 600).await?;

    let event = serde_json::to_string(&WorldmapUpdatedEvent {
        r#type: "worldmap_updated",
        room_id,
        version: next_version,
    })
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to serialize event: {err}"),
        )
    })?;

    state.ws_hub.broadcast(room_id, event).await;
    info!(user_id = %user.user_id, room_id = %room_id, version = next_version, "worldmap updated");

    Ok((
        StatusCode::CREATED,
        Json(WorldmapResponse {
            version: next_version,
            asset_id: payload.asset_id,
            download_url,
            created_at: Utc::now(),
        }),
    ))
}

pub async fn get_worldmap(
    State(state): State<AppState>,
    Path(room_id): Path<Uuid>,
    Extension(user): Extension<AuthenticatedUser>,
) -> Result<Json<WorldmapResponse>, ApiError> {
    ensure_room_member(&state.db_pool, room_id, user.user_id).await?;

    let row = sqlx::query_as::<_, WorldmapRecord>(
        "SELECT w.version, w.asset_id, w.created_at, a.storage_path
         FROM worldmaps w
         JOIN assets a ON a.id = w.asset_id
         WHERE w.room_id = $1
         ORDER BY w.version DESC
         LIMIT 1",
    )
    .bind(room_id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(internal_error)?
    .ok_or((StatusCode::NOT_FOUND, "worldmap not found".to_string()))?;

    let download_url = supabase_storage::create_signed_download_url(&state, &row.storage_path, 600).await?;

    Ok(Json(WorldmapResponse {
        version: row.version,
        asset_id: row.asset_id,
        download_url,
        created_at: row.created_at,
    }))
}
