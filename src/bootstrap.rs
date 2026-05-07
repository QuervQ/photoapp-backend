use aws_config::BehaviorVersion;
use aws_credential_types::Credentials;
use aws_sdk_s3::config::Region;
use std::{net::SocketAddr, sync::Arc};

// use aws_credential_types::Credentials;

use crate::{
    app_state::{AppState, StorageConfig, WsHub},
    config::AppConfig,
    db, router,
};

pub async fn run() {
    let config = AppConfig::from_env();
    let db_pool = db::connect(&config.database_url).await;
    db::migrate(&db_pool).await;

    let credentials = Credentials::new(
        config.storage_access_key.clone(),
        config.storage_secret_key.clone(),
        None,
        None,
        "static",
    );

    let shared_config = aws_config::defaults(BehaviorVersion::latest())
        .credentials_provider(credentials)
        .region(Region::new(config.storage_region.clone()))
        .load()
        .await;

    let mut s3_builder = aws_sdk_s3::config::Builder::from(&shared_config)
        .force_path_style(config.storage_path_style);

    if !config.storage_endpoint.trim().is_empty() {
        s3_builder = s3_builder.endpoint_url(config.storage_endpoint.clone());
    }

    let storage_client = aws_sdk_s3::Client::from_conf(s3_builder.build());

    let state = AppState {
        db_pool,
        ws_hub: Arc::new(WsHub::new()),
        jwt_secret: config.jwt_secret,
        storage: StorageConfig {
            endpoint: config.storage_endpoint,
            region: config.storage_region,
            access_key: config.storage_access_key,
            secret_key: config.storage_secret_key,
            bucket: config.storage_bucket,
            path_style: config.storage_path_style,
        },
        storage_client,
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
