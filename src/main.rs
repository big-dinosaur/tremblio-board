mod auth;
mod config;
mod db;
mod error;
mod models;
mod routes;

use std::sync::Arc;

use sqlx::AnyPool;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub db: AnyPool,
    pub config: Arc<Config>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tremblio_board=info,tower_http=info".into()),
        )
        .init();

    let config = Config::from_env()?;
    let db = db::connect(&config.database_url).await?;
    tracing::info!("数据库已连接并完成迁移");

    let state = AppState {
        db,
        config: Arc::new(config.clone()),
    };
    let app = routes::router()
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    tracing::info!("监听 http://{}", config.bind_addr);
    axum::serve(listener, app).await?;
    Ok(())
}
