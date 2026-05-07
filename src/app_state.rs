use std::{collections::HashMap, sync::Arc};

use aws_sdk_s3::Client as S3Client;
use sqlx::PgPool;
use tokio::sync::{Mutex, broadcast};
use uuid::Uuid;

#[derive(Clone)]
#[allow(dead_code)]
pub struct StorageConfig {
    pub endpoint: String,
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
    pub bucket: String,
    pub path_style: bool,
}

#[derive(Clone)]
pub struct AppState {
    pub db_pool: PgPool,
    pub ws_hub: Arc<WsHub>,
    pub jwt_secret: String,
    pub storage: StorageConfig,
    pub storage_client: S3Client,
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
