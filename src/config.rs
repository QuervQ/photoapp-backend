use std::env;

pub struct AppConfig {
    pub port: u16,
    pub database_url: String,
    pub jwt_secret: String,
    pub ws_allowed_origins: String,
    pub storage_endpoint: String,
    pub storage_region: String,
    pub storage_access_key: String,
    pub storage_secret_key: String,
    pub storage_bucket: String,
    pub storage_path_style: bool,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let port = env::var("PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(8080);

        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL is required");
        let jwt_secret = env::var("JWT_SECRET").expect("JWT_SECRET is required");
        let ws_allowed_origins = env::var("WS_ALLOWED_ORIGINS")
            .unwrap_or_else(|_| "http://localhost:3000,http://127.0.0.1:3000".to_string());
        let storage_endpoint = env::var("STORAGE_ENDPOINT").unwrap_or_default();
        let storage_region =
            env::var("STORAGE_REGION").unwrap_or_else(|_| "us-east-1".to_string());
        let storage_access_key =
            env::var("STORAGE_ACCESS_KEY").expect("STORAGE_ACCESS_KEY is required");
        let storage_secret_key =
            env::var("STORAGE_SECRET_KEY").expect("STORAGE_SECRET_KEY is required");
        let storage_bucket = env::var("STORAGE_BUCKET").expect("STORAGE_BUCKET is required");
        let storage_path_style = env::var("STORAGE_PATH_STYLE")
            .ok()
            .map(|value| value == "true" || value == "1")
            .unwrap_or(true);

        Self {
            port,
            database_url,
            jwt_secret,
            ws_allowed_origins,
            storage_endpoint,
            storage_region,
            storage_access_key,
            storage_secret_key,
            storage_bucket,
            storage_path_style,
        }
    }
}
