mod config;
mod conversion;
mod events;
mod jobs;
mod stats;
mod system;
mod types;

pub use events::*;
pub use types::*;

use crate::error::{AlchemistError, Result};
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
use sqlx::SqlitePool;
use std::time::Duration;
use tokio::time::timeout;
use tracing::info;

/// Default timeout for potentially slow database queries
pub(crate) const QUERY_TIMEOUT: Duration = Duration::from_secs(5);

/// Execute a query with a timeout to prevent blocking the job loop
pub(crate) async fn timed_query<T, F, Fut>(operation: &str, f: F) -> Result<T>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    match timeout(QUERY_TIMEOUT, f()).await {
        Ok(result) => result,
        Err(_) => Err(AlchemistError::QueryTimeout(
            QUERY_TIMEOUT.as_secs(),
            operation.to_string(),
        )),
    }
}

#[derive(Clone, Debug)]
pub(crate) struct WatchDirSchemaFlags {
    has_is_recursive: bool,
    has_recursive: bool,
    has_enabled: bool,
    has_profile_id: bool,
}

#[derive(Clone, Debug)]
pub struct Db {
    pub(crate) pool: SqlitePool,
    pub(crate) watch_dir_flags: std::sync::Arc<WatchDirSchemaFlags>,
}

impl Db {
    pub async fn new(db_path: &str) -> Result<Self> {
        let start = std::time::Instant::now();
        let options = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(5));

        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        info!(
            target: "startup",
            "Database connection opened in {} ms",
            start.elapsed().as_millis()
        );

        // Run migrations
        let migrate_start = std::time::Instant::now();
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|e| crate::error::AlchemistError::Database(e.into()))?;
        info!(
            target: "startup",
            "Database migrations completed in {} ms",
            migrate_start.elapsed().as_millis()
        );

        // Cache watch_dirs schema flags once at startup to avoid repeated PRAGMA queries.
        let check = |column: &str| {
            let pool = pool.clone();
            let column = column.to_string();
            async move {
                let row =
                    sqlx::query("SELECT name FROM pragma_table_info('watch_dirs') WHERE name = ?")
                        .bind(&column)
                        .fetch_optional(&pool)
                        .await
                        .unwrap_or(None);
                row.is_some()
            }
        };
        let watch_dir_flags = WatchDirSchemaFlags {
            has_is_recursive: check("is_recursive").await,
            has_recursive: check("recursive").await,
            has_enabled: check("enabled").await,
            has_profile_id: check("profile_id").await,
        };

        Ok(Self {
            pool,
            watch_dir_flags: std::sync::Arc::new(watch_dir_flags),
        })
    }
}

/// Hash a session token using SHA256 for secure storage.
///
/// # Security: Timing Attack Resistance
///
/// Session tokens are hashed before storage and lookup. Token validation uses
/// SQL `WHERE token = ?` with the hashed value, so the comparison occurs in
/// SQLite rather than in Rust code. This is inherently constant-time from the
/// application's perspective because:
/// 1. The database performs the comparison, not our code
/// 2. Database query time doesn't leak information about partial matches
/// 3. No early-exit comparison in application code
///
/// This design makes timing attacks infeasible without requiring the `subtle`
/// crate for constant-time comparison.
pub(crate) fn hash_session_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(&mut out, "{:02x}", byte);
    }
    out
}

pub fn hash_api_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(&mut out, "{:02x}", byte);
    }
    out
}
