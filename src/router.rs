use axum::{
    middleware,
    http::header::HeaderName,
    Router,
    routing::{get, post},
};
use tower_http::{
    request_id::{
        MakeRequestUuid,
        PropagateRequestIdLayer,
        SetRequestIdLayer,
    },
    trace::TraceLayer,
};

use crate::{
    app_state::AppState,
    auth_middleware,
    cors,
    routes,
};

/// Axumルーターを構築する。公開ルート（ヘルスチェック・認証・WS）と認証必須ルート（ルーム・アセット・Worldmap・配置）を定義。
pub fn build_router(state: AppState, ws_allowed_origins: &str) -> Router {
    let request_id_header = HeaderName::from_static("x-request-id");

    let public_routes = Router::new()
        .route("/healthz", get(routes::health::healthz))
        .route("/auth/signup", post(routes::auth::signup))
        .route("/auth/login", post(routes::auth::login))
        .route("/auth/refresh", post(routes::auth::refresh))
        .route("/ws", get(routes::ws::ws_handler));

    let protected_routes = Router::new()
        .route("/me", get(routes::auth::me))
        .route("/rooms", post(routes::rooms::create_room).get(routes::rooms::list_rooms))
        .route("/rooms/{room_id}/invite", post(routes::rooms::create_invite))
        .route("/rooms/join", post(routes::rooms::join_room))
        .route("/assets/upload-url", post(routes::assets::create_upload_url))
        .route("/assets/{asset_id}/download-url", get(routes::assets::get_download_url))
        .route(
            "/rooms/{room_id}/worldmap",
            post(routes::worldmap::set_worldmap).get(routes::worldmap::get_worldmap),
        )
        .route(
            "/rooms/{room_id}/placements",
            post(routes::rooms::create_placement).get(routes::rooms::list_placements),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware::require_auth,
        ));

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .layer(PropagateRequestIdLayer::new(request_id_header.clone()))
        .layer(SetRequestIdLayer::new(
            request_id_header,
            MakeRequestUuid,
        ))
        .layer(TraceLayer::new_for_http())
        .layer(cors::build_cors_layer(ws_allowed_origins))
        .with_state(state)
}
