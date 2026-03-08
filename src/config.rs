use std::env;

/// アプリケーション設定。環境変数から読み込む。
pub struct AppConfig {
    pub port: u16,
    pub database_url: String,
    pub supabase_jwt_secret: String,
    pub ws_allowed_origins: String,
    pub supabase_url: String,
    pub supabase_anon_key: String,
    pub supabase_service_role_key: String,
    pub supabase_storage_bucket: String,
}

impl AppConfig {
    /// 環境変数から各設定値を読み込んでAppConfigを構築する。
    pub fn from_env() -> Self {
        let port = env::var("PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(8080);

        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL is required");
        let supabase_jwt_secret =
            env::var("SUPABASE_JWT_SECRET").expect("SUPABASE_JWT_SECRET is required");
        let ws_allowed_origins = env::var("WS_ALLOWED_ORIGINS")
            .unwrap_or_else(|_| "http://localhost:3000,http://127.0.0.1:3000".to_string());
        let supabase_url = env::var("SUPABASE_URL").expect("SUPABASE_URL is required");
        let supabase_anon_key =
            env::var("SUPABASE_ANON_KEY").expect("SUPABASE_ANON_KEY is required");
        let supabase_service_role_key =
            env::var("SUPABASE_SERVICE_ROLE_KEY").expect("SUPABASE_SERVICE_ROLE_KEY is required");
        let supabase_storage_bucket =
            env::var("SUPABASE_STORAGE_BUCKET").expect("SUPABASE_STORAGE_BUCKET is required");

        Self {
            port,
            database_url,
            supabase_jwt_secret,
            ws_allowed_origins,
            supabase_url,
            supabase_anon_key,
            supabase_service_role_key,
            supabase_storage_bucket,
        }
    }
}
