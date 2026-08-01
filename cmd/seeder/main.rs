//! HOA TCMS — Standalone seed binary.
//!
//! Seeds permissions, system groups, built-in roles, and role-permission
//! associations without starting the HTTP server.
//!
//! Usage:
//!   DATABASE_URL=postgres://... cargo run --bin seeder
//!
//! The `SETUP_CONFIG_PATH` env var overrides the default config file path
//! (default: `config/default-setup.yaml`).

use tracing::Level;

use hoa_tcms_core::application::repositories::setup_seeder::SetupSeeder;

#[tokio::main]
async fn main() {
    // Initialise minimal logging.
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default tracing subscriber failed");

    // Load .env for DATABASE_URL.
    dotenvy::dotenv().ok();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let config_path = std::env::var("SETUP_CONFIG_PATH")
        .unwrap_or_else(|_| "config/default-setup.yaml".to_string());

    tracing::info!(%config_path, "loading setup configuration");

    // Create database connection pool.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .expect("failed to connect to database");

    let db = sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool);

    // Load and run the seeder.
    let config =
        hoa_tcms_core::infrastructure::config::setup_seeder::ConfigFileSetupSeeder::load_config(
            &config_path,
        )
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "failed to load setup config; using empty defaults");
            panic!("cannot seed without a valid setup config: {e}");
        });

    let seeder = hoa_tcms_core::infrastructure::config::setup_seeder::ConfigFileSetupSeeder::new(
        &db, config,
    );

    match seeder.seed().await {
        Ok(()) => {
            tracing::info!("seed complete");
            println!("Seed complete.");
        }
        Err(e) => {
            tracing::error!(error = %e, "seed failed");
            panic!("seed failed: {e}");
        }
    }
}
