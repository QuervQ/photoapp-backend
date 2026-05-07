use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::{
    api_error::{ApiError, internal_error},
    app_state::AppState,
    auth_middleware::AuthenticatedUser,
    supabase_storage,
};
use tracing::info;

#[derive(Deserialize)]
pub struct CreateRoomRequest {
    name: String,
}

#[derive(Serialize)]
pub struct RoomResponse {
    id: Uuid,
    name: String,
    created_by: Uuid,
    created_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct RoomRecord {
    id: Uuid,
    name: String,
    created_by: Uuid,
    created_at: DateTime<Utc>,
}

/// 新規ルームを作成し、作成者をメンバーとして登録する。
pub async fn create_room(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(payload): Json<CreateRoomRequest>,
) -> Result<(StatusCode, Json<RoomResponse>), ApiError> {
    let user_id = user.user_id;

    if payload.name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "room name is required".to_string()));
    }

    let room_id = Uuid::new_v4();
    let created = sqlx::query(
        "INSERT INTO rooms (id, name, created_by) VALUES ($1, $2, $3)
		 ON CONFLICT DO NOTHING",
    )
    .bind(room_id)
    .bind(payload.name.trim())
    .bind(user_id)
    .execute(&state.db_pool)
    .await
    .map_err(internal_error)?;

    if created.rows_affected() == 0 {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to create room".to_string(),
        ));
    }

    sqlx::query(
        "INSERT INTO room_members (room_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(room_id)
    .bind(user_id)
    .execute(&state.db_pool)
    .await
    .map_err(internal_error)?;

    let room = sqlx::query_as::<_, RoomRecord>(
        "SELECT id, name, created_by, created_at FROM rooms WHERE id = $1",
    )
    .bind(room_id)
    .fetch_one(&state.db_pool)
    .await
    .map_err(internal_error)?;

    Ok((
        StatusCode::CREATED,
        Json(RoomResponse {
            id: room.id,
            name: room.name,
            created_by: room.created_by,
            created_at: room.created_at,
        }),
    ))
}

/// 認証ユーザーが参加中のルーム一覧を取得する。
pub async fn list_rooms(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
) -> Result<Json<Vec<RoomResponse>>, ApiError> {
    let user_id = user.user_id;

    let rooms = sqlx::query_as::<_, RoomRecord>(
        "SELECT r.id, r.name, r.created_by, r.created_at
		 FROM rooms r
		 JOIN room_members m ON r.id = m.room_id
		 WHERE m.user_id = $1
		 ORDER BY r.created_at DESC",
    )
    .bind(user_id)
    .fetch_all(&state.db_pool)
    .await
    .map_err(internal_error)?;

    Ok(Json(
        rooms
            .into_iter()
            .map(|room| RoomResponse {
                id: room.id,
                name: room.name,
                created_by: room.created_by,
                created_at: room.created_at,
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
pub struct JoinRoomRequest {
    invite_code: String,
}

#[derive(Serialize)]
pub struct JoinRoomResponse {
    room_id: Uuid,
}

/// 招待コードを使ってルームに参加する。コードの有効期限を検証。
pub async fn join_room(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(payload): Json<JoinRoomRequest>,
) -> Result<Json<JoinRoomResponse>, ApiError> {
    let user_id = user.user_id;

    let room_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT room_id
		 FROM room_invites
		 WHERE code = $1 AND expires_at > NOW()",
    )
    .bind(payload.invite_code.trim())
    .fetch_optional(&state.db_pool)
    .await
    .map_err(internal_error)?
    .ok_or((StatusCode::NOT_FOUND, "invalid invite code".to_string()))?;

    sqlx::query(
        "INSERT INTO room_members (room_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(room_id)
    .bind(user_id)
    .execute(&state.db_pool)
    .await
    .map_err(internal_error)?;

    info!(user_id = %user_id, room_id = %room_id, "joined room by invite");
    Ok(Json(JoinRoomResponse { room_id }))
}

#[derive(Serialize)]
pub struct InviteResponse {
    invite_code: String,
    expires_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct InviteRecord {
    code: String,
    expires_at: DateTime<Utc>,
}

/// ルームの招待コードを生成する（24時間有効）。ルームメンバーのみ実行可。
pub async fn create_invite(
    State(state): State<AppState>,
    Path(room_id): Path<Uuid>,
    Extension(user): Extension<AuthenticatedUser>,
) -> Result<Json<InviteResponse>, ApiError> {
    let user_id = user.user_id;
    ensure_room_member(&state.db_pool, room_id, user_id).await?;

    let invite_code = Uuid::new_v4().simple().to_string()[..12].to_string();

    let record = sqlx::query_as::<_, InviteRecord>(
        "INSERT INTO room_invites (code, room_id, created_by, expires_at)
		 VALUES ($1, $2, $3, NOW() + INTERVAL '24 hours')
		 RETURNING code, expires_at",
    )
    .bind(invite_code)
    .bind(room_id)
    .bind(user_id)
    .fetch_one(&state.db_pool)
    .await
    .map_err(internal_error)?;

    info!(user_id = %user_id, room_id = %room_id, "invite generated");
    Ok(Json(InviteResponse {
        invite_code: record.code,
        expires_at: record.expires_at,
    }))
}

#[derive(Deserialize)]
pub struct CreatePlacementRequest {
    image_asset_id: Uuid,
    transform: Vec<f64>,
    width_m: f64,
    height_m: f64,
}

#[derive(Serialize, Clone)]
pub struct PlacementResponse {
    id: Uuid,
    room_id: Uuid,
    image_asset_id: Uuid,
    transform: Vec<f64>,
    width_m: f64,
    height_m: f64,
    created_by: Uuid,
    created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_download_url: Option<String>,
}

#[derive(FromRow)]
struct PlacementRecord {
    id: Uuid,
    room_id: Uuid,
    image_asset_id: Uuid,
    transform: Vec<f64>,
    width_m: f64,
    height_m: f64,
    created_by: Uuid,
    created_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct PlacementWithPathRecord {
    id: Uuid,
    room_id: Uuid,
    image_asset_id: Uuid,
    transform: Vec<f64>,
    width_m: f64,
    height_m: f64,
    created_by: Uuid,
    created_at: DateTime<Utc>,
    storage_path: String,
}

#[derive(Serialize)]
struct PlacementCreatedEvent {
    r#type: &'static str,
    room_id: Uuid,
    placement: PlacementResponse,
}

/// AR配置（プレイスメント）を作成しDBに保存。作成後、WebSocketでルーム全体にブロードキャスト。
pub async fn create_placement(
    State(state): State<AppState>,
    Path(room_id): Path<Uuid>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(payload): Json<CreatePlacementRequest>,
) -> Result<(StatusCode, Json<PlacementResponse>), ApiError> {
    let user_id = user.user_id;
    ensure_room_member(&state.db_pool, room_id, user_id).await?;

    if payload.transform.len() != 16 {
        return Err((
            StatusCode::BAD_REQUEST,
            "transform must have 16 elements".to_string(),
        ));
    }

    let placement_id = Uuid::new_v4();
    let placement = sqlx::query_as::<_, PlacementRecord>(
        "INSERT INTO placements
		 (id, room_id, image_asset_id, transform, width_m, height_m, created_by)
		 VALUES ($1, $2, $3, $4, $5, $6, $7)
		 RETURNING id, room_id, image_asset_id, transform, width_m, height_m, created_by, created_at",
    )
    .bind(placement_id)
    .bind(room_id)
    .bind(payload.image_asset_id)
    .bind(payload.transform)
    .bind(payload.width_m)
    .bind(payload.height_m)
    .bind(user_id)
    .fetch_one(&state.db_pool)
    .await
    .map_err(internal_error)?;

    let placement_response = PlacementResponse {
        id: placement.id,
        room_id: placement.room_id,
        image_asset_id: placement.image_asset_id,
        transform: placement.transform,
        width_m: placement.width_m,
        height_m: placement.height_m,
        created_by: placement.created_by,
        created_at: placement.created_at,
        image_download_url: None,
    };

    // WS配信用: 画像のダウンロードURLを含める（リモート端末がAR描画に使用）
    let storage_path =
        sqlx::query_scalar::<_, String>("SELECT storage_path FROM assets WHERE id = $1")
            .bind(payload.image_asset_id)
            .fetch_optional(&state.db_pool)
            .await
            .map_err(internal_error)?;

    let mut broadcast_placement = placement_response.clone();
    if let Some(path) = storage_path {
        broadcast_placement.image_download_url =
            supabase_storage::create_signed_download_url(&state, &path, 600)
                .await
                .ok();
    }

    let event = serde_json::to_string(&PlacementCreatedEvent {
        r#type: "placement_created",
        room_id,
        placement: broadcast_placement,
    })
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to serialize event: {err}"),
        )
    })?;

    state.ws_hub.broadcast(room_id, event).await;
    info!(user_id = %user_id, room_id = %room_id, placement_id = %placement_response.id, "placement created");

    Ok((StatusCode::CREATED, Json(placement_response)))
}

/// ルーム内の全配置を取得する。各配置に画像の署名付きダウンロードURLを付与。
pub async fn list_placements(
    State(state): State<AppState>,
    Path(room_id): Path<Uuid>,
    Extension(user): Extension<AuthenticatedUser>,
) -> Result<Json<Vec<PlacementResponse>>, ApiError> {
    let user_id = user.user_id;
    ensure_room_member(&state.db_pool, room_id, user_id).await?;

    let placements = sqlx::query_as::<_, PlacementWithPathRecord>(
        "SELECT p.id, p.room_id, p.image_asset_id, p.transform, p.width_m, p.height_m,
		        p.created_by, p.created_at, a.storage_path
		 FROM placements p
		 JOIN assets a ON a.id = p.image_asset_id
		 WHERE p.room_id = $1
		 ORDER BY p.created_at ASC",
    )
    .bind(room_id)
    .fetch_all(&state.db_pool)
    .await
    .map_err(internal_error)?;

    let mut results = Vec::with_capacity(placements.len());
    for p in placements {
        let download_url =
            supabase_storage::create_signed_download_url(&state, &p.storage_path, 600)
                .await
                .ok();
        results.push(PlacementResponse {
            id: p.id,
            room_id: p.room_id,
            image_asset_id: p.image_asset_id,
            transform: p.transform,
            width_m: p.width_m,
            height_m: p.height_m,
            created_by: p.created_by,
            created_at: p.created_at,
            image_download_url: download_url,
        });
    }

    Ok(Json(results))
}

/// ユーザーが指定ルームのメンバーであることを検証する。メンバーでなければ403を返す。
pub async fn ensure_room_member(
    db_pool: &PgPool,
    room_id: Uuid,
    user_id: Uuid,
) -> Result<(), ApiError> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(
			SELECT 1 FROM room_members WHERE room_id = $1 AND user_id = $2
		)",
    )
    .bind(room_id)
    .bind(user_id)
    .fetch_one(db_pool)
    .await
    .map_err(internal_error)?;

    if !exists {
        return Err((StatusCode::FORBIDDEN, "not a room member".to_string()));
    }

    Ok(())
}
