use sqlx::{
    PgPool,
    postgres::PgPoolOptions,
};

pub async fn connect(database_url: &str) -> PgPool {
    PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
        .expect("failed to connect to database")
}

pub async fn migrate(pool: &PgPool) {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .expect("failed to run database migrations");
}
