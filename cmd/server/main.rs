//! HOA TCMS — Server entry point.
//!
//! This binary starts the Actix-Web HTTP server with configuration loaded
//! from environment variables. It creates the shared application state
//! (database pool, Redis client) and registers it with the HTTP server
//! so all handlers can access it via `web::Data<AppState>`.

use actix_web::web;
use sea_orm::SqlxPostgresConnector;
use std::net::TcpListener;
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

use hoa_tcms_core::infrastructure::config::AppState;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // -----------------------------------------------------------------------
    // Initialise logging.
    // -----------------------------------------------------------------------
    let max_level = std::env::var("LOG_MAX_LEVEL")
        .ok()
        .and_then(|s| s.parse::<Level>().ok())
        .unwrap_or(Level::TRACE);

    let subscriber = FmtSubscriber::builder()
        .with_max_level(max_level)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default tracing subscriber failed");

    // -----------------------------------------------------------------------
    // Load environment variables from `.env` (development) or system env.
    // -----------------------------------------------------------------------
    dotenvy::dotenv().ok();

    // Reject known-default secrets before starting.
    let session_secret = std::env::var("SESSION_SECRET").unwrap_or_default();
    if session_secret == "CHANGE_ME_TO_A_RANDOM_SECRET" || session_secret.len() < 32 {
        panic!(
            "SESSION_SECRET is insecure: must be changed from the default \
             and be at least 32 characters. Generate one with: \
             openssl rand -base64 32"
        );
    }

    // -----------------------------------------------------------------------
    // Resolve listen address.
    // -----------------------------------------------------------------------
    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse::<u16>()
        .expect("PORT must be a valid u16");

    let listen_addr = format!("{}:{}", host, port);
    let listener = TcpListener::bind(&listen_addr)
        .unwrap_or_else(|e| panic!("failed to bind to {}: {}", listen_addr, e));

    info!(
        "server starting on http://{}",
        listener.local_addr().unwrap()
    );

    // -----------------------------------------------------------------------
    // Create database connection pool.
    // -----------------------------------------------------------------------
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let max_connections = std::env::var("DB_MAX_CONNECTIONS")
        .unwrap_or_else(|_| "10".to_string())
        .parse::<u32>()
        .expect("DB_MAX_CONNECTIONS must be a valid u32");

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(max_connections)
        .connect(&database_url)
        .await
        .expect("failed to create PostgreSQL connection pool");

    info!(max_connections, "database connection pool established");

    // -----------------------------------------------------------------------
    // Create SeaORM connection wrapper around the PgPool.
    // -----------------------------------------------------------------------
    let db = SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());

    info!("SeaORM connection wrapper created");

    // -----------------------------------------------------------------------
    // Create Redis client.
    // -----------------------------------------------------------------------
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");

    let redis_client = redis::Client::open(redis_url.as_str())
        .expect("invalid REDIS_URL: must be a valid Redis connection URI");

    info!("Redis client configured");

    // -----------------------------------------------------------------------
    // Build and run the server with shared application state.
    // -----------------------------------------------------------------------
    let app_state = web::Data::new(AppState {
        pool,
        db,
        redis_client,
    });

    actix_web::HttpServer::new(move || {
        actix_web::App::new()
            .app_data(app_state.clone())
            .configure(hoa_tcms_core::configure_app)
    })
    .listen(listener)?
    .run()
    .await
}
