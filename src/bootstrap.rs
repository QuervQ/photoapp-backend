use std::{net::SocketAddr, sync::Arc};

use crate::{
    app_state::{AppState, SupabaseConfig, WsHub},
    config::AppConfig,
    db,
    router,
};

pub async fn run() {
    let config = AppConfig::from_env();
    let db_pool = db::connect(&config.database_url).await;
    db::migrate(&db_pool).await;

    let state = AppState {
        db_pool,
        ws_hub: Arc::new(WsHub::new()),
        jwt_secret: config.supabase_jwt_secret,
        http_client: reqwest::Client::new(),
        supabase: SupabaseConfig {
            url: config.supabase_url,
            anon_key: config.supabase_anon_key,
            service_role_key: config.supabase_service_role_key,
            storage_bucket: config.supabase_storage_bucket,
        },
    };

    let app = router::build_router(state, &config.ws_allowed_origins);
    let address = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!("starting backend on {}", address);

    let listener = tokio::net::TcpListener::bind(address)
        .await
        .expect("failed to bind tcp listener");

    axum::serve(listener, app)
        .await
        .expect("failed to start axum server");
}
