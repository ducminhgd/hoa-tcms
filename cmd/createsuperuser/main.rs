//! HOA TCMS — createsuperuser
//!
//! Creates a user and adds them to the "System Admin" group, granting
//! full system access (all permissions via admin bypass).
//!
//! Usage:
//!   DATABASE_URL=postgres://... cargo run --bin createsuperuser -- \
//!     --username admin --email admin@example.com \
//!     --password changeme --fullname "System Administrator"
//!
//! If `--password` is omitted, a prompt reads it from stdin securely.
//!
//! Uses raw sqlx to avoid SeaORM chrono type-mapping issues
//! (TIMESTAMPTZ <-> NaiveDateTime).

use std::io::{self, Write};

use sqlx::PgPool;
use tracing::Level;

use hoa_tcms_core::application::services::password_hasher::PasswordHasher;
use hoa_tcms_core::infrastructure::crypto::pbkdf2_hasher::Pbkdf2Hasher;

// OWASP 2023 recommended minimum for PBKDF2-HMAC-SHA256.
const PBKDF2_ITERATIONS: u32 = 600_000;

struct Args {
    username: String,
    email: String,
    password: Option<String>,
    fullname: String,
}

fn parse_args() -> Result<Args, String> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut args = Args {
        username: String::new(),
        email: String::new(),
        password: None,
        fullname: String::new(),
    };

    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--username" => {
                i += 1;
                if i >= raw.len() {
                    return Err("--username requires a value".into());
                }
                args.username = raw[i].clone();
            }
            "--email" => {
                i += 1;
                if i >= raw.len() {
                    return Err("--email requires a value".into());
                }
                args.email = raw[i].clone();
            }
            "--password" => {
                i += 1;
                if i >= raw.len() {
                    return Err("--password requires a value".into());
                }
                args.password = Some(raw[i].clone());
            }
            "--fullname" => {
                i += 1;
                if i >= raw.len() {
                    return Err("--fullname requires a value".into());
                }
                args.fullname = raw[i].clone();
            }
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            other => return Err(format!("unknown flag: {other}")),
        }
        i += 1;
    }

    if args.username.is_empty() || args.email.is_empty() || args.fullname.is_empty() {
        return Err(
            "--username, --email, and --fullname are required. Use --help for usage.".into(),
        );
    }

    Ok(args)
}

fn print_usage() {
    eprintln!(
        "\
Usage: createsuperuser [OPTIONS]

Options:
  --username STR      Username for the superuser (required)
  --email STR         Email address (required)
  --password STR      Password (optional — prompts if omitted)
  --fullname STR      Full display name (required)
  -h, --help          Print this help"
    );
}

fn read_password() -> Result<String, String> {
    eprint!("Password: ");
    io::stderr().flush().map_err(|e| e.to_string())?;
    let password = rpassword::read_password().map_err(|e| e.to_string())?;
    eprint!("Confirm password: ");
    io::stderr().flush().map_err(|e| e.to_string())?;
    let confirm = rpassword::read_password().map_err(|e| e.to_string())?;
    if password != confirm {
        return Err("passwords do not match".into());
    }
    Ok(password)
}

#[tokio::main]
async fn main() {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default tracing subscriber failed");

    dotenvy::dotenv().ok();

    let args = parse_args().unwrap_or_else(|e| {
        eprintln!("error: {e}");
        print_usage();
        std::process::exit(1);
    });

    let password = args.password.unwrap_or_else(|| {
        read_password().unwrap_or_else(|e| {
            eprintln!("error reading password: {e}");
            std::process::exit(1);
        })
    });

    if password.len() < 8 {
        eprintln!("error: password must be at least 8 characters");
        std::process::exit(1);
    }

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    // -----------------------------------------------------------------------
    // Connect to the database.
    // -----------------------------------------------------------------------
    let pool = PgPool::connect(&database_url)
        .await
        .expect("failed to connect to database");

    // -----------------------------------------------------------------------
    // Hash the password.
    // -----------------------------------------------------------------------
    let hasher = Pbkdf2Hasher::new(PBKDF2_ITERATIONS);
    let password_hash = hasher
        .hash(&password)
        .await
        .expect("failed to hash password");

    // -----------------------------------------------------------------------
    // Insert the user directly via raw SQL (avoids SeaORM chrono mismatch).
    // -----------------------------------------------------------------------
    let user_row: (i64,) = sqlx::query_as(
        r#"INSERT INTO users (username, email, password_hash, fullname, status, created_by, updated_by)
           VALUES ($1, $2, $3, $4, 'ACTIVE', NULL, NULL)
           RETURNING id"#,
    )
    .bind(&args.username)
    .bind(&args.email)
    .bind(&password_hash)
    .bind(&args.fullname)
    .fetch_one(&pool)
    .await
    .expect("failed to create user");

    let user_id = user_row.0;
    tracing::info!(id = user_id, username = args.username, "user created");

    // -----------------------------------------------------------------------
    // Add user to the "System Admin" group.
    // -----------------------------------------------------------------------
    let (group_id,): (i64,) =
        sqlx::query_as("SELECT id FROM groups WHERE name = 'System Admin' AND deleted_at IS NULL")
            .fetch_one(&pool)
            .await
            .expect("System Admin group not found — run 'make seed' first");

    sqlx::query("INSERT INTO user_groups (user_id, group_id) VALUES ($1, $2)")
        .bind(user_id)
        .bind(group_id)
        .execute(&pool)
        .await
        .expect("failed to add user to System Admin group");

    tracing::info!(
        user_id,
        group_id,
        group_name = "System Admin",
        "user added to System Admin group",
    );

    println!(
        "Superuser '{}' (id={}) created and added to System Admin group.",
        args.username, user_id
    );
}
