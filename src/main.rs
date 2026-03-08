mod api_error;
mod app_state;
mod auth_middleware;
mod bootstrap;
mod config;
mod cors;
mod db;
mod router;
mod routes;
mod security;
mod supabase_storage;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    bootstrap::run().await;
}
