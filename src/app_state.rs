use std::{collections::HashMap, sync::Arc};

use sqlx::PgPool;
use tokio::sync::{Mutex, broadcast};
use uuid::Uuid;

/// Supabase接続に必要な設定値をまとめた構造体。
#[derive(Clone)]
pub struct SupabaseConfig {
    pub url: String,
    pub anon_key: String,
    pub service_role_key: String,
    pub storage_bucket: String,
}

/// アプリケーション全体で共有される状態。DB接続プール、WebSocketハブ、JWT秘密鍵、HTTPクライアント等を保持。
#[derive(Clone)]
pub struct AppState {
    pub db_pool: PgPool,
    pub ws_hub: Arc<WsHub>,
    pub jwt_secret: String,
    pub http_client: reqwest::Client,
    pub supabase: SupabaseConfig,
}

/// ルームごとのWebSocket配信チャネルを管理するハブ。
pub struct WsHub {
    rooms: Mutex<HashMap<Uuid, broadcast::Sender<String>>>,
}

impl WsHub {
    /// 空のWsHubを生成する。
    pub fn new() -> Self {
        Self {
            rooms: Mutex::new(HashMap::new()),
        }
    }

    /// 指定ルームのbroadcastチャネルを購読する。チャネルが無ければ新規作成。
    pub async fn subscribe(&self, room_id: Uuid) -> broadcast::Receiver<String> {
        let mut rooms = self.rooms.lock().await;
        if let Some(sender) = rooms.get(&room_id) {
            return sender.subscribe();
        }

        let (sender, receiver) = broadcast::channel(256);
        rooms.insert(room_id, sender);
        receiver
    }

    /// 指定ルームの全購読者にメッセージをブロードキャストする。
    pub async fn broadcast(&self, room_id: Uuid, payload: String) {
        let mut rooms = self.rooms.lock().await;
        let sender = rooms
            .entry(room_id)
            .or_insert_with(|| broadcast::channel(256).0);
        let _ = sender.send(payload);
    }
}
