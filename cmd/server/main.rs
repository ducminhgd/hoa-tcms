//! HOA TCMS — Server entry point.
//!
//! This binary starts the Actix-Web HTTP server with configuration loaded
//! from environment variables. It creates the shared application state
//! (database pool, Redis client) and registers it with the HTTP server
//! so all handlers can access it via `web::Data<AppState>`.

use actix_web::web;
use sea_orm::SqlxPostgresConnector;
use std::net::TcpListener;
use std::sync::Arc;
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

use hoa_tcms_core::adapters::http::handlers::project_handler::ProjectHandler;
use hoa_tcms_core::adapters::http::handlers::test_execution_handler::TestExecutionHandler;
use hoa_tcms_core::application::services::authorization::AuthorizationService;
use hoa_tcms_core::application::services::project_service::ProjectService;
use hoa_tcms_core::application::services::test_execution_service::TestExecutionService;
use hoa_tcms_core::configure_app;
use hoa_tcms_core::infrastructure::config::AppState;
use hoa_tcms_core::infrastructure::config::metadata_seeder::ConfigFileSeeder;
use hoa_tcms_core::infrastructure::postgres::repositories::admin_bypass_repository::SqlAdminBypassRepository;
use hoa_tcms_core::infrastructure::postgres::repositories::permission_resolver::SqlPermissionResolver;
use hoa_tcms_core::infrastructure::postgres::repositories::project_member_repository::SqlProjectMemberRepository;
use hoa_tcms_core::infrastructure::postgres::repositories::project_repository::SqlProjectRepository;
use hoa_tcms_core::infrastructure::postgres::repositories::test_case_result_repository::SqlTestCaseResultRepository;
use hoa_tcms_core::infrastructure::postgres::repositories::test_execution_repository::SqlTestExecutionRepository;
use hoa_tcms_core::infrastructure::redis::session_store::RedisSessionStore;

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
    // Create Redis client and session store.
    // -----------------------------------------------------------------------
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");

    let redis_client = redis::Client::open(redis_url.as_str())
        .expect("invalid REDIS_URL: must be a valid Redis connection URI");

    info!("Redis client configured");

    let redis_connection = redis_client
        .get_multiplexed_async_connection()
        .await
        .expect("failed to establish Redis multiplexed connection");

    let session_store: Arc<dyn hoa_tcms_core::application::services::session_store::SessionStore> =
        Arc::new(RedisSessionStore::new(redis_connection));

    // -----------------------------------------------------------------------
    // Wire up dependencies (Clean Architecture DI).
    // -----------------------------------------------------------------------

    // Authorization service.
    let permission_resolver = Box::new(SqlPermissionResolver::new(db.clone()));
    let admin_bypass_repo = Box::new(SqlAdminBypassRepository::new(db.clone()));
    let auth_service = Arc::new(AuthorizationService::new(
        permission_resolver,
        admin_bypass_repo,
    ));

    // Load metadata config (used by both seeder and project repository).
    let config_path = std::env::var("METADATA_CONFIG_PATH")
        .unwrap_or_else(|_| "config/default-metadata.yaml".to_string());
    let metadata_config = ConfigFileSeeder::load_config(&config_path).unwrap_or_else(|e| {
        tracing::warn!(error = %e, "failed to load metadata config; using empty defaults");
        ConfigFileSeeder::empty_config()
    });

    // Project service dependencies.
    let project_repo = Box::new(SqlProjectRepository::new(db.clone(), metadata_config));
    let member_repo = Box::new(SqlProjectMemberRepository::new(db.clone()));

    // Config seeder (no longer used for create — create_project_transactional handles seeding).
    let seeder = Box::new(ConfigFileSeeder::new(db.clone(), &config_path).await);

    let project_service = Arc::new(ProjectService::new(
        project_repo,
        member_repo,
        seeder,
        auth_service.clone(),
    ));

    let project_handler = Arc::new(ProjectHandler::new(project_service));

    // Test execution service dependencies.
    let execution_repo = Box::new(SqlTestExecutionRepository::new(db.clone()));
    let result_repo = Box::new(SqlTestCaseResultRepository::new(db.clone()));

    let test_execution_service = Arc::new(TestExecutionService::new(
        execution_repo,
        result_repo,
        auth_service.clone(),
    ));

    let test_execution_handler = Arc::new(TestExecutionHandler::new(test_execution_service));

    // -----------------------------------------------------------------------
    // Build and run the server with shared application state.
    // -----------------------------------------------------------------------
    let app_state = web::Data::new(AppState {
        pool: pool.clone(),
        db: db.clone(),
        redis_client: redis_client.clone(),
        session_store: session_store.clone(),
        project_handler: project_handler.clone(),
        test_execution_handler: test_execution_handler.clone(),
    });

    let session_store_for_app = session_store.clone();
    let project_handler_for_app = project_handler.clone();
    let test_execution_handler_for_app = test_execution_handler.clone();

    actix_web::HttpServer::new(move || {
        actix_web::App::new()
            .app_data(app_state.clone())
            .app_data(web::Data::from(session_store_for_app.clone()))
            .configure(|cfg| {
                configure_app(
                    cfg,
                    project_handler_for_app.clone(),
                    test_execution_handler_for_app.clone(),
                    session_store_for_app.clone(),
                )
            })
    })
    .listen(listener)?
    .run()
    .await
}
