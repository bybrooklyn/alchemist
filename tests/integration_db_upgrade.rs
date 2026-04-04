use alchemist::db::{Db, JobState};
use anyhow::{Context, Result};
use sqlx::{
    Row,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::fs;
use std::path::PathBuf;

fn upgrade_fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("db_v0_2_5.sqlite")
}

fn temp_db_copy() -> Result<PathBuf> {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "alchemist_upgrade_test_{}.db",
        rand::random::<u64>()
    ));
    fs::copy(upgrade_fixture_path(), &path)?;
    Ok(path)
}

#[tokio::test]
async fn v0_2_5_fixture_upgrades_and_preserves_core_state() -> Result<()> {
    let db_path = temp_db_copy()?;
    let db = Db::new(db_path.to_string_lossy().as_ref()).await?;

    let user = db
        .get_user_by_username("upgrade-admin")
        .await?
        .context("expected seeded user")?;
    assert_eq!(user.username, "upgrade-admin");

    let watch_dirs = db.get_watch_dirs().await?;
    assert_eq!(watch_dirs.len(), 1);
    assert_eq!(watch_dirs[0].path, "/srv/media");
    assert!(watch_dirs[0].is_recursive);

    let file_settings = db.get_file_settings().await?;
    assert_eq!(file_settings.output_extension, "mp4");
    assert_eq!(file_settings.output_suffix, "-legacy");
    assert_eq!(file_settings.replace_strategy, "replace");
    assert_eq!(file_settings.output_root, None);

    let notifications = db.get_notification_targets().await?;
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].target_type, "discord");

    let schedule_windows = db.get_schedule_windows().await?;
    assert_eq!(schedule_windows.len(), 1);
    assert_eq!(schedule_windows[0].start_time, "22:00");

    assert_eq!(
        db.get_preference("active_theme_id").await?,
        Some("midnight".to_string())
    );

    let job = db
        .get_job_by_input_path("/srv/media/movies/example.mkv")
        .await?
        .context("expected seeded job")?;
    assert_eq!(job.status, JobState::Completed);
    assert_eq!(
        job.decision_reason.as_deref(),
        Some("Legacy AV1 skip threshold")
    );

    let stats = db.get_aggregated_stats().await?;
    assert_eq!(stats.total_jobs, 1);
    assert_eq!(stats.completed_jobs, 1);
    assert_eq!(stats.total_input_size, 1_000_000);
    assert_eq!(stats.total_output_size, 700_000);

    let profiles = db.get_all_profiles().await?;
    assert_eq!(profiles.len(), 4);
    assert!(profiles.iter().any(|profile| profile.name == "Balanced"));

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(SqliteConnectOptions::new().filename(&db_path))
        .await?;

    let schema_version: String =
        sqlx::query("SELECT value FROM schema_info WHERE key = 'schema_version'")
            .fetch_one(&pool)
            .await?
            .get("value");
    assert_eq!(schema_version, "5");

    let min_compatible_version: String =
        sqlx::query("SELECT value FROM schema_info WHERE key = 'min_compatible_version'")
            .fetch_one(&pool)
            .await?
            .get("value");
    assert_eq!(min_compatible_version, "0.2.5");

    let file_settings_columns = sqlx::query("PRAGMA table_info(file_settings)")
        .fetch_all(&pool)
        .await?
        .into_iter()
        .map(|row| row.get::<String, _>("name"))
        .collect::<Vec<_>>();
    assert!(
        file_settings_columns
            .iter()
            .any(|name| name == "output_root")
    );

    let jobs_columns = sqlx::query("PRAGMA table_info(jobs)")
        .fetch_all(&pool)
        .await?
        .into_iter()
        .map(|row| row.get::<String, _>("name"))
        .collect::<Vec<_>>();
    assert!(jobs_columns.iter().any(|name| name == "archived"));
    assert!(jobs_columns.iter().any(|name| name == "health_issues"));
    assert!(jobs_columns.iter().any(|name| name == "last_health_check"));

    pool.close().await;
    drop(db);
    let _ = fs::remove_file(&db_path);
    Ok(())
}
