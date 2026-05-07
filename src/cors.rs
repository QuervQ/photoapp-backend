use axum::http::{HeaderValue, Method};
use tower_http::cors::{Any, CorsLayer};

/// CORSレイヤーを構築する。"*"なら全オリジン許可、それ以外はカンマ区切りで許可リストを設定。
pub fn build_cors_layer(allowed_origins: &str) -> CorsLayer {
    let mut layer = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    if allowed_origins.trim() == "*" {
        layer = layer.allow_origin(Any);
        return layer;
    }

    let parsed: Vec<HeaderValue> = allowed_origins
        .split(',')
        .filter_map(|origin| HeaderValue::from_str(origin.trim()).ok())
        .collect();

    if parsed.is_empty() {
        layer.allow_origin(Any)
    } else {
        layer.allow_origin(parsed)
    }
}
