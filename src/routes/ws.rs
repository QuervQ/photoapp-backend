use axum::{
	extract::{Query, State, WebSocketUpgrade, ws::Message},
	http::HeaderMap,
	response::IntoResponse,
};
use serde::Deserialize;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::{api_error::ApiError, app_state::AppState};
use tracing::info;

use crate::auth_middleware::extract_user_id_from_headers;

use super::rooms::ensure_room_member;

#[derive(Deserialize)]
pub struct WsQuery {
	room_id: Option<Uuid>,
	token: Option<String>,
}

pub async fn ws_handler(
	ws: WebSocketUpgrade,
	State(state): State<AppState>,
	headers: HeaderMap,
	Query(query): Query<WsQuery>,
) -> Result<impl IntoResponse, ApiError> {
	let user_id = extract_user_id_from_headers(&headers, &state.jwt_secret, query.token.as_deref())?;
	Ok(ws.on_upgrade(move |socket| handle_ws_socket(socket, state, user_id, query.room_id)))
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum WsClientMessage {
	#[serde(rename = "subscribe")]
	Subscribe { room_id: Uuid },
}

async fn handle_ws_socket(
	mut socket: axum::extract::ws::WebSocket,
	state: AppState,
	user_id: Uuid,
	initial_room: Option<Uuid>,
) {
	let mut receiver = if let Some(room_id) = initial_room {
		match ensure_room_member(&state.db_pool, room_id, user_id).await {
			Ok(_) => {
				info!(user_id = %user_id, room_id = %room_id, "ws subscribed from query room");
				Some(state.ws_hub.subscribe(room_id).await)
			}
			Err(err) => {
				let _ = socket
					.send(Message::Text(
						format!("{{\"type\":\"error\",\"message\":\"{}\"}}", err.1).into(),
					))
					.await;
				None
			}
		}
	} else {
		None
	};

	loop {
		if let Some(rx) = receiver.as_mut() {
			tokio::select! {
				inbound = socket.recv() => {
					if !handle_inbound(inbound, &state, user_id, &mut receiver, &mut socket).await {
						break;
					}
				}
				outbound = rx.recv() => {
					match outbound {
						Ok(payload) => {
							if socket.send(Message::Text(payload.into())).await.is_err() {
								break;
							}
						}
						Err(_) => break,
					}
				}
			}
		} else {
			let inbound = socket.recv().await;
			if !handle_inbound(inbound, &state, user_id, &mut receiver, &mut socket).await {
				break;
			}
		}
	}
}

async fn handle_inbound(
	inbound: Option<Result<Message, axum::Error>>,
	state: &AppState,
	user_id: Uuid,
	receiver: &mut Option<broadcast::Receiver<String>>,
	socket: &mut axum::extract::ws::WebSocket,
) -> bool {
	let Some(Ok(message)) = inbound else {
		return false;
	};

	match message {
		Message::Text(text) => {
			let parsed = serde_json::from_str::<WsClientMessage>(&text);
			match parsed {
				Ok(WsClientMessage::Subscribe { room_id }) => {
					if ensure_room_member(&state.db_pool, room_id, user_id)
						.await
						.is_ok()
					{
						info!(user_id = %user_id, room_id = %room_id, "ws subscribed from message");
						*receiver = Some(state.ws_hub.subscribe(room_id).await);
						let _ = socket
							.send(Message::Text(
								format!("{{\"type\":\"subscribed\",\"room_id\":\"{}\"}}", room_id)
									.into(),
							))
							.await;
					} else {
						let _ = socket
							.send(Message::Text(
								"{\"type\":\"error\",\"message\":\"not a room member\"}".into(),
							))
							.await;
					}
				}
				Err(_) => {
					let _ = socket
						.send(Message::Text(
							"{\"type\":\"error\",\"message\":\"invalid message\"}".into(),
						))
						.await;
				}
			}
		}
		Message::Close(_) => return false,
		_ => {}
	}

	true
}
