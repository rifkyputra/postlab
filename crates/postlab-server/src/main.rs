use anyhow::Result;
use axum::{
    routing::{delete, get, post, put},
    Router,
};
use postlab_core::db;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod routes;
mod state;

use state::AppState;

#[tokio::main]
async fn main() -> Result<()> {
    // ── Logging ───────────────────────────────────────────────────────────────
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // ── Database ──────────────────────────────────────────────────────────────
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://postlab.db?mode=rwc".to_string());

    tracing::info!("Connecting to database: {db_url}");
    let pool = db::connect(&db_url).await?;
    db::migrate(&pool).await?;
    tracing::info!("Migrations applied");

    // ── App state ─────────────────────────────────────────────────────────────
    let state = AppState::new(pool);

    // ── Router ────────────────────────────────────────────────────────────────
    let app = Router::new()
        // Servers
        .route("/api/servers", get(routes::servers::list_servers))
        .route("/api/servers", post(routes::servers::create_server))
        .route("/api/servers/:id", get(routes::servers::get_server))
        .route("/api/servers/:id", delete(routes::servers::delete_server))
        .route("/api/servers/:id/status", get(routes::servers::server_status))
        // Server actions
        .route("/api/servers/:id/install", post(routes::install::install_app))
        .route("/api/servers/:id/upgrade", post(routes::install::upgrade_os))
        .route("/api/servers/:id/harden", post(routes::install::harden))
        // Tasks
        .route("/api/tasks", get(routes::tasks::list_tasks))
        .route("/api/tasks", post(routes::tasks::create_task))
        .route("/api/tasks/:id", get(routes::tasks::get_task))
        // Audit
        .route("/api/audit", get(routes::audit::list_audit))
        // Config
        .route("/api/config", get(routes::config::list_config))
        .route("/api/config/:key", put(routes::config::set_config))
        // Middleware
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = "0.0.0.0:3000";
    tracing::info!("Listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
