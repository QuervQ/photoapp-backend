use std::{collections::HashMap, sync::Arc};

use sqlx::PgPool;
use tokio::sync::{Mutex, broadcast};
use uuid::Uuid;

#[derive(Clone)]
pub struct SupabaseConfig {
    pub url: String,
    pub service_role_key: String,
    pub storage_bucket: String,
}

#[derive(Clone)]
pub struct AppState {
    pub db_pool: PgPool,
    pub ws_hub: Arc<WsHub>,
    pub jwt_secret: String,
    pub http_client: reqwest::Client,
    pub supabase: SupabaseConfig,
}

pub struct WsHub {
    rooms: Mutex<HashMap<Uuid, broadcast::Sender<String>>>,
}

impl WsHub {
    pub fn new() -> Self {
        Self {
            rooms: Mutex::new(HashMap::new()),
        }
    }

    pub async fn subscribe(&self, room_id: Uuid) -> broadcast::Receiver<String> {
        let mut rooms = self.rooms.lock().await;
        if let Some(sender) = rooms.get(&room_id) {
            return sender.subscribe();
        }

        let (sender, receiver) = broadcast::channel(256);
        rooms.insert(room_id, sender);
        receiver
    }

    pub async fn broadcast(&self, room_id: Uuid, payload: String) {
        let mut rooms = self.rooms.lock().await;
        let sender = rooms
            .entry(room_id)
            .or_insert_with(|| broadcast::channel(256).0);
        let _ = sender.send(payload);
    }
}
