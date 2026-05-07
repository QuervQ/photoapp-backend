use axum::http::StatusCode;
use tracing::warn;

/// APIエラーの型エイリアス。HTTPステータスコードとエラーメッセージの組。
pub type ApiError = (StatusCode, String);

/// SQLxのDBエラーを500 Internal Server Errorに変換するヘルパー。
pub fn internal_error(err: sqlx::Error) -> ApiError {
    warn!("database error: {err}");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal server error".to_string(),
    )
}
