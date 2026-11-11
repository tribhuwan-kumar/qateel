use std::path::Path;
use std::str::FromStr;
use std::time::Duration;
use tracing::{info, debug};
use sqlx::{
    Pool, Sqlite,
    ConnectOptions,
    sqlite::{
        SqlitePoolOptions,
        SqliteJournalMode,
        SqliteConnectOptions
    }
};

pub const DB_TIMEOUT: u64 = 16;
pub const MAX_DB_CONNECTIONS: u32 = 16;
pub const MIN_DB_CONNECTIONS: u32 = 1;

pub async fn init_db(data_dir: &Path) -> Result<Pool<Sqlite>, sqlx::Error> {
    let db_path = data_dir.join("qateel.db");
    if !db_path.exists() {
        std::fs::File::create(&db_path)?;
    }

	debug!("Database path: {:?}", std::fs::canonicalize(&db_path));

    let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());

    let conc_options = SqliteConnectOptions::from_str(&db_url)?
		.foreign_keys(true)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(DB_TIMEOUT))
        .disable_statement_logging();

    let pool = SqlitePoolOptions::new()
        .max_connections(MAX_DB_CONNECTIONS)
		.min_connections(MIN_DB_CONNECTIONS)
        .acquire_timeout(Duration::from_secs(DB_TIMEOUT))
        .connect_with(conc_options)
        .await?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await?;

    info!("Database initialized successfully with WAL mode & foreign keys!!");

    Ok(pool)
}

