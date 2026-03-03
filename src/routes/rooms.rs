use axum::{
	Extension,
	Json,
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

#[derive(Serialize)]
struct PlacementCreatedEvent {
	r#type: &'static str,
	room_id: Uuid,
	placement: PlacementResponse,
}

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
	};

	let event = serde_json::to_string(&PlacementCreatedEvent {
		r#type: "placement_created",
		room_id,
		placement: placement_response.clone(),
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

pub async fn list_placements(
	State(state): State<AppState>,
	Path(room_id): Path<Uuid>,
	Extension(user): Extension<AuthenticatedUser>,
) -> Result<Json<Vec<PlacementResponse>>, ApiError> {
	let user_id = user.user_id;
	ensure_room_member(&state.db_pool, room_id, user_id).await?;

	let placements = sqlx::query_as::<_, PlacementRecord>(
		"SELECT id, room_id, image_asset_id, transform, width_m, height_m, created_by, created_at
		 FROM placements
		 WHERE room_id = $1
		 ORDER BY created_at ASC",
	)
	.bind(room_id)
	.fetch_all(&state.db_pool)
	.await
	.map_err(internal_error)?;

	Ok(Json(
		placements
			.into_iter()
			.map(|p| PlacementResponse {
				id: p.id,
				room_id: p.room_id,
				image_asset_id: p.image_asset_id,
				transform: p.transform,
				width_m: p.width_m,
				height_m: p.height_m,
				created_by: p.created_by,
				created_at: p.created_at,
			})
			.collect(),
	))
}

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
