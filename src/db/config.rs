use crate::error::Result;
use serde_json::Value as JsonValue;
use sqlx::Row;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::Db;
use super::types::*;

fn notification_config_string(config_json: &str, key: &str) -> Option<String> {
    serde_json::from_str::<JsonValue>(config_json)
        .ok()
        .and_then(|value| {
            value
                .get(key)
                .and_then(JsonValue::as_str)
                .map(str::to_string)
        })
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn notification_legacy_columns(
    target_type: &str,
    config_json: &str,
) -> (String, Option<String>, Option<String>) {
    match target_type {
        "discord_webhook" => (
            "discord".to_string(),
            notification_config_string(config_json, "webhook_url"),
            None,
        ),
        "discord_bot" => (
            "discord".to_string(),
            Some("https://discord.com".to_string()),
            notification_config_string(config_json, "bot_token"),
        ),
        "gotify" => (
            "gotify".to_string(),
            notification_config_string(config_json, "server_url"),
            notification_config_string(config_json, "app_token"),
        ),
        "webhook" => (
            "webhook".to_string(),
            notification_config_string(config_json, "url"),
            notification_config_string(config_json, "auth_token"),
        ),
        "telegram" => (
            "webhook".to_string(),
            Some("https://api.telegram.org".to_string()),
            notification_config_string(config_json, "bot_token"),
        ),
        "email" => ("webhook".to_string(), None, None),
        other => (other.to_string(), None, None),
    }
}

impl Db {
    pub async fn get_watch_dirs(&self) -> Result<Vec<WatchDir>> {
        let has_is_recursive = self.watch_dir_flags.has_is_recursive;
        let has_recursive = self.watch_dir_flags.has_recursive;
        let has_enabled = self.watch_dir_flags.has_enabled;
        let has_profile_id = self.watch_dir_flags.has_profile_id;

        let recursive_expr = if has_is_recursive {
            "is_recursive"
        } else if has_recursive {
            "recursive"
        } else {
            "1"
        };

        let enabled_filter = if has_enabled {
            "WHERE enabled = 1 "
        } else {
            ""
        };
        let profile_expr = if has_profile_id { "profile_id" } else { "NULL" };
        let query = format!(
            "SELECT id, path, {} as is_recursive, {} as profile_id, created_at
             FROM watch_dirs {}ORDER BY path ASC",
            recursive_expr, profile_expr, enabled_filter
        );

        let dirs = sqlx::query_as::<_, WatchDir>(&query)
            .fetch_all(&self.pool)
            .await?;
        Ok(dirs)
    }

    pub async fn add_watch_dir(&self, path: &str, is_recursive: bool) -> Result<WatchDir> {
        let has_is_recursive = self.watch_dir_flags.has_is_recursive;
        let has_recursive = self.watch_dir_flags.has_recursive;
        let has_profile_id = self.watch_dir_flags.has_profile_id;

        let row = if has_is_recursive && has_profile_id {
            sqlx::query_as::<_, WatchDir>(
                "INSERT INTO watch_dirs (path, is_recursive) VALUES (?, ?)
                 RETURNING id, path, is_recursive, profile_id, created_at",
            )
            .bind(path)
            .bind(is_recursive)
            .fetch_one(&self.pool)
            .await?
        } else if has_is_recursive {
            sqlx::query_as::<_, WatchDir>(
                "INSERT INTO watch_dirs (path, is_recursive) VALUES (?, ?)
                 RETURNING id, path, is_recursive, NULL as profile_id, created_at",
            )
            .bind(path)
            .bind(is_recursive)
            .fetch_one(&self.pool)
            .await?
        } else if has_recursive && has_profile_id {
            sqlx::query_as::<_, WatchDir>(
                "INSERT INTO watch_dirs (path, recursive) VALUES (?, ?)
                 RETURNING id, path, recursive as is_recursive, profile_id, created_at",
            )
            .bind(path)
            .bind(is_recursive)
            .fetch_one(&self.pool)
            .await?
        } else if has_recursive {
            sqlx::query_as::<_, WatchDir>(
                "INSERT INTO watch_dirs (path, recursive) VALUES (?, ?)
                 RETURNING id, path, recursive as is_recursive, NULL as profile_id, created_at",
            )
            .bind(path)
            .bind(is_recursive)
            .fetch_one(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, WatchDir>(
                "INSERT INTO watch_dirs (path) VALUES (?)
                 RETURNING id, path, 1 as is_recursive, NULL as profile_id, created_at",
            )
            .bind(path)
            .fetch_one(&self.pool)
            .await?
        };
        Ok(row)
    }

    pub async fn replace_watch_dirs(
        &self,
        watch_dirs: &[crate::config::WatchDirConfig],
    ) -> Result<()> {
        let has_is_recursive = self.watch_dir_flags.has_is_recursive;
        let has_recursive = self.watch_dir_flags.has_recursive;
        let has_profile_id = self.watch_dir_flags.has_profile_id;
        let preserved_profiles = if has_profile_id {
            let rows = sqlx::query("SELECT path, profile_id FROM watch_dirs")
                .fetch_all(&self.pool)
                .await?;
            rows.into_iter()
                .map(|row| {
                    let path: String = row.get("path");
                    let profile_id: Option<i64> = row.get("profile_id");
                    (path, profile_id)
                })
                .collect::<HashMap<_, _>>()
        } else {
            HashMap::new()
        };
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM watch_dirs")
            .execute(&mut *tx)
            .await?;
        for watch_dir in watch_dirs {
            let preserved_profile_id = preserved_profiles.get(&watch_dir.path).copied().flatten();
            if has_is_recursive && has_profile_id {
                sqlx::query(
                    "INSERT INTO watch_dirs (path, is_recursive, profile_id) VALUES (?, ?, ?)",
                )
                .bind(&watch_dir.path)
                .bind(watch_dir.is_recursive)
                .bind(preserved_profile_id)
                .execute(&mut *tx)
                .await?;
            } else if has_recursive && has_profile_id {
                sqlx::query(
                    "INSERT INTO watch_dirs (path, recursive, profile_id) VALUES (?, ?, ?)",
                )
                .bind(&watch_dir.path)
                .bind(watch_dir.is_recursive)
                .bind(preserved_profile_id)
                .execute(&mut *tx)
                .await?;
            } else if has_recursive {
                sqlx::query("INSERT INTO watch_dirs (path, recursive) VALUES (?, ?)")
                    .bind(&watch_dir.path)
                    .bind(watch_dir.is_recursive)
                    .execute(&mut *tx)
                    .await?;
            } else {
                sqlx::query("INSERT INTO watch_dirs (path) VALUES (?)")
                    .bind(&watch_dir.path)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn remove_watch_dir(&self, id: i64) -> Result<()> {
        let res = sqlx::query("DELETE FROM watch_dirs WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if res.rows_affected() == 0 {
            return Err(crate::error::AlchemistError::Database(
                sqlx::Error::RowNotFound,
            ));
        }
        Ok(())
    }

    pub async fn get_all_profiles(&self) -> Result<Vec<LibraryProfile>> {
        let profiles = sqlx::query_as::<_, LibraryProfile>(
            "SELECT id, name, preset, codec, quality_profile, hdr_mode, audio_mode,
                    crf_override, notes, created_at, updated_at
             FROM library_profiles
             ORDER BY id ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(profiles)
    }

    pub async fn get_profile(&self, id: i64) -> Result<Option<LibraryProfile>> {
        let profile = sqlx::query_as::<_, LibraryProfile>(
            "SELECT id, name, preset, codec, quality_profile, hdr_mode, audio_mode,
                    crf_override, notes, created_at, updated_at
             FROM library_profiles
             WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(profile)
    }

    pub async fn create_profile(&self, profile: NewLibraryProfile) -> Result<i64> {
        let id = sqlx::query(
            "INSERT INTO library_profiles
                (name, preset, codec, quality_profile, hdr_mode, audio_mode, crf_override, notes, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)",
        )
        .bind(profile.name)
        .bind(profile.preset)
        .bind(profile.codec)
        .bind(profile.quality_profile)
        .bind(profile.hdr_mode)
        .bind(profile.audio_mode)
        .bind(profile.crf_override)
        .bind(profile.notes)
        .execute(&self.pool)
        .await?
        .last_insert_rowid();
        Ok(id)
    }

    pub async fn update_profile(&self, id: i64, profile: NewLibraryProfile) -> Result<()> {
        let result = sqlx::query(
            "UPDATE library_profiles
             SET name = ?,
                 preset = ?,
                 codec = ?,
                 quality_profile = ?,
                 hdr_mode = ?,
                 audio_mode = ?,
                 crf_override = ?,
                 notes = ?,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?",
        )
        .bind(profile.name)
        .bind(profile.preset)
        .bind(profile.codec)
        .bind(profile.quality_profile)
        .bind(profile.hdr_mode)
        .bind(profile.audio_mode)
        .bind(profile.crf_override)
        .bind(profile.notes)
        .bind(id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(crate::error::AlchemistError::Database(
                sqlx::Error::RowNotFound,
            ));
        }

        Ok(())
    }

    pub async fn delete_profile(&self, id: i64) -> Result<()> {
        let result = sqlx::query("DELETE FROM library_profiles WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(crate::error::AlchemistError::Database(
                sqlx::Error::RowNotFound,
            ));
        }
        Ok(())
    }

    pub async fn assign_profile_to_watch_dir(
        &self,
        dir_id: i64,
        profile_id: Option<i64>,
    ) -> Result<()> {
        let result = sqlx::query(
            "UPDATE watch_dirs
             SET profile_id = ?
             WHERE id = ?",
        )
        .bind(profile_id)
        .bind(dir_id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(crate::error::AlchemistError::Database(
                sqlx::Error::RowNotFound,
            ));
        }

        Ok(())
    }

    pub async fn get_profile_for_path(&self, path: &str) -> Result<Option<LibraryProfile>> {
        let normalized = Path::new(path);
        let candidate = sqlx::query_as::<_, LibraryProfile>(
            "SELECT lp.id, lp.name, lp.preset, lp.codec, lp.quality_profile, lp.hdr_mode,
                    lp.audio_mode, lp.crf_override, lp.notes, lp.created_at, lp.updated_at
             FROM watch_dirs wd
             JOIN library_profiles lp ON lp.id = wd.profile_id
             WHERE wd.profile_id IS NOT NULL
               AND (
                    ? = wd.path
                    OR (
                        length(?) > length(wd.path)
                        AND (
                            substr(?, 1, length(wd.path) + 1) = wd.path || '/'
                            OR substr(?, 1, length(wd.path) + 1) = wd.path || '\\'
                        )
                    )
               )
             ORDER BY LENGTH(wd.path) DESC
             LIMIT 1",
        )
        .bind(path)
        .bind(path)
        .bind(path)
        .bind(path)
        .fetch_optional(&self.pool)
        .await?;

        if candidate.is_some() {
            return Ok(candidate);
        }

        // SQLite prefix matching is a fast first pass; fall back to strict path ancestry
        // if separators or normalization differ.
        let rows = sqlx::query(
            "SELECT wd.path,
                    lp.id, lp.name, lp.preset, lp.codec, lp.quality_profile, lp.hdr_mode,
                    lp.audio_mode, lp.crf_override, lp.notes, lp.created_at, lp.updated_at
             FROM watch_dirs wd
             JOIN library_profiles lp ON lp.id = wd.profile_id
             WHERE wd.profile_id IS NOT NULL",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut best: Option<(usize, LibraryProfile)> = None;
        for row in rows {
            let watch_path: String = row.get("path");
            let profile = LibraryProfile {
                id: row.get("id"),
                name: row.get("name"),
                preset: row.get("preset"),
                codec: row.get("codec"),
                quality_profile: row.get("quality_profile"),
                hdr_mode: row.get("hdr_mode"),
                audio_mode: row.get("audio_mode"),
                crf_override: row.get("crf_override"),
                notes: row.get("notes"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            };
            let watch_path_buf = PathBuf::from(&watch_path);
            if normalized == watch_path_buf || normalized.starts_with(&watch_path_buf) {
                let score = watch_path.len();
                if best
                    .as_ref()
                    .is_none_or(|(best_score, _)| score > *best_score)
                {
                    best = Some((score, profile));
                }
            }
        }

        Ok(best.map(|(_, profile)| profile))
    }

    pub async fn count_watch_dirs_using_profile(&self, profile_id: i64) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM watch_dirs WHERE profile_id = ?")
            .bind(profile_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    pub async fn get_notification_targets(&self) -> Result<Vec<NotificationTarget>> {
        let flags = &self.notification_target_flags;
        let targets = if flags.has_target_type_v2 {
            sqlx::query_as::<_, NotificationTarget>(
                "SELECT
                    id,
                    name,
                    COALESCE(
                        NULLIF(target_type_v2, ''),
                        CASE target_type
                            WHEN 'discord' THEN 'discord_webhook'
                            WHEN 'gotify' THEN 'gotify'
                            ELSE 'webhook'
                        END
                    ) AS target_type,
                    CASE
                        WHEN trim(config_json) != '' THEN config_json
                        WHEN target_type = 'discord' THEN json_object('webhook_url', endpoint_url)
                        WHEN target_type = 'gotify' THEN json_object('server_url', endpoint_url, 'app_token', COALESCE(auth_token, ''))
                        ELSE json_object('url', endpoint_url, 'auth_token', auth_token)
                    END AS config_json,
                    events,
                    enabled,
                    created_at
                 FROM notification_targets
                 ORDER BY id ASC",
            )
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, NotificationTarget>(
                "SELECT id, name, target_type, config_json, events, enabled, created_at
                 FROM notification_targets
                 ORDER BY id ASC",
            )
            .fetch_all(&self.pool)
            .await?
        };
        Ok(targets)
    }

    pub async fn add_notification_target(
        &self,
        name: &str,
        target_type: &str,
        config_json: &str,
        events: &str,
        enabled: bool,
    ) -> Result<NotificationTarget> {
        let flags = &self.notification_target_flags;
        if flags.has_target_type_v2 {
            let (legacy_target_type, endpoint_url, auth_token) =
                notification_legacy_columns(target_type, config_json);
            let result = sqlx::query(
                "INSERT INTO notification_targets
                    (name, target_type, target_type_v2, endpoint_url, auth_token, config_json, events, enabled)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(name)
            .bind(legacy_target_type)
            .bind(target_type)
            .bind(endpoint_url)
            .bind(auth_token)
            .bind(config_json)
            .bind(events)
            .bind(enabled)
            .execute(&self.pool)
            .await?;
            self.get_notification_target_by_id(result.last_insert_rowid())
                .await
        } else {
            let result = sqlx::query(
                "INSERT INTO notification_targets (name, target_type, config_json, events, enabled)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(name)
            .bind(target_type)
            .bind(config_json)
            .bind(events)
            .bind(enabled)
            .execute(&self.pool)
            .await?;
            self.get_notification_target_by_id(result.last_insert_rowid())
                .await
        }
    }

    pub async fn delete_notification_target(&self, id: i64) -> Result<()> {
        let res = sqlx::query("DELETE FROM notification_targets WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if res.rows_affected() == 0 {
            return Err(crate::error::AlchemistError::Database(
                sqlx::Error::RowNotFound,
            ));
        }
        Ok(())
    }

    pub async fn replace_notification_targets(
        &self,
        targets: &[crate::config::NotificationTargetConfig],
    ) -> Result<()> {
        let flags = &self.notification_target_flags;
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM notification_targets")
            .execute(&mut *tx)
            .await?;
        for target in targets {
            let config_json = target.config_json.to_string();
            let events = serde_json::to_string(&target.events).unwrap_or_else(|_| "[]".to_string());
            if flags.has_target_type_v2 {
                let (legacy_target_type, endpoint_url, auth_token) =
                    notification_legacy_columns(&target.target_type, &config_json);
                sqlx::query(
                    "INSERT INTO notification_targets
                        (name, target_type, target_type_v2, endpoint_url, auth_token, config_json, events, enabled)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(&target.name)
                .bind(legacy_target_type)
                .bind(&target.target_type)
                .bind(endpoint_url)
                .bind(auth_token)
                .bind(&config_json)
                .bind(&events)
                .bind(target.enabled)
                .execute(&mut *tx)
                .await?;
            } else {
                sqlx::query(
                    "INSERT INTO notification_targets (name, target_type, config_json, events, enabled) VALUES (?, ?, ?, ?, ?)",
                )
                .bind(&target.name)
                .bind(&target.target_type)
                .bind(&config_json)
                .bind(&events)
                .bind(target.enabled)
                .execute(&mut *tx)
                .await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }

    async fn get_notification_target_by_id(&self, id: i64) -> Result<NotificationTarget> {
        let flags = &self.notification_target_flags;
        let row = if flags.has_target_type_v2 {
            sqlx::query_as::<_, NotificationTarget>(
                "SELECT
                    id,
                    name,
                    COALESCE(
                        NULLIF(target_type_v2, ''),
                        CASE target_type
                            WHEN 'discord' THEN 'discord_webhook'
                            WHEN 'gotify' THEN 'gotify'
                            ELSE 'webhook'
                        END
                    ) AS target_type,
                    CASE
                        WHEN trim(config_json) != '' THEN config_json
                        WHEN target_type = 'discord' THEN json_object('webhook_url', endpoint_url)
                        WHEN target_type = 'gotify' THEN json_object('server_url', endpoint_url, 'app_token', COALESCE(auth_token, ''))
                        ELSE json_object('url', endpoint_url, 'auth_token', auth_token)
                    END AS config_json,
                    events,
                    enabled,
                    created_at
                 FROM notification_targets
                 WHERE id = ?",
            )
            .bind(id)
            .fetch_one(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, NotificationTarget>(
                "SELECT id, name, target_type, config_json, events, enabled, created_at
                 FROM notification_targets
                 WHERE id = ?",
            )
            .bind(id)
            .fetch_one(&self.pool)
            .await?
        };
        Ok(row)
    }

    pub async fn get_schedule_windows(&self) -> Result<Vec<ScheduleWindow>> {
        let windows =
            sqlx::query_as::<_, ScheduleWindow>("SELECT * FROM schedule_windows ORDER BY id ASC")
                .fetch_all(&self.pool)
                .await?;
        Ok(windows)
    }

    pub async fn add_schedule_window(
        &self,
        start_time: &str,
        end_time: &str,
        days_of_week: &str,
        enabled: bool,
    ) -> Result<ScheduleWindow> {
        let row = sqlx::query_as::<_, ScheduleWindow>(
            "INSERT INTO schedule_windows (start_time, end_time, days_of_week, enabled)
            VALUES (?, ?, ?, ?)
            RETURNING *",
        )
        .bind(start_time)
        .bind(end_time)
        .bind(days_of_week)
        .bind(enabled)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn delete_schedule_window(&self, id: i64) -> Result<()> {
        let res = sqlx::query("DELETE FROM schedule_windows WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if res.rows_affected() == 0 {
            return Err(crate::error::AlchemistError::Database(
                sqlx::Error::RowNotFound,
            ));
        }
        Ok(())
    }

    pub async fn replace_schedule_windows(
        &self,
        windows: &[crate::config::ScheduleWindowConfig],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM schedule_windows")
            .execute(&mut *tx)
            .await?;
        for window in windows {
            sqlx::query(
                "INSERT INTO schedule_windows (start_time, end_time, days_of_week, enabled) VALUES (?, ?, ?, ?)",
            )
            .bind(&window.start_time)
            .bind(&window.end_time)
            .bind(serde_json::to_string(&window.days_of_week).unwrap_or_else(|_| "[]".to_string()))
            .bind(window.enabled)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn get_file_settings(&self) -> Result<FileSettings> {
        // Migration ensures row 1 exists, but we handle missing just in case
        let row = sqlx::query_as::<_, FileSettings>("SELECT * FROM file_settings WHERE id = 1")
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(s) => Ok(s),
            None => {
                // If missing (shouldn't happen), return default
                Ok(FileSettings {
                    id: 1,
                    delete_source: false,
                    output_extension: "mkv".to_string(),
                    output_suffix: "-alchemist".to_string(),
                    replace_strategy: "keep".to_string(),
                    output_root: None,
                })
            }
        }
    }

    pub async fn update_file_settings(
        &self,
        delete_source: bool,
        output_extension: &str,
        output_suffix: &str,
        replace_strategy: &str,
        output_root: Option<&str>,
    ) -> Result<FileSettings> {
        let row = sqlx::query_as::<_, FileSettings>(
            "UPDATE file_settings
            SET delete_source = ?, output_extension = ?, output_suffix = ?, replace_strategy = ?, output_root = ?
            WHERE id = 1
            RETURNING *",
        )
        .bind(delete_source)
        .bind(output_extension)
        .bind(output_suffix)
        .bind(replace_strategy)
        .bind(output_root)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn replace_file_settings_projection(
        &self,
        settings: &crate::config::FileSettingsConfig,
    ) -> Result<FileSettings> {
        self.update_file_settings(
            settings.delete_source,
            &settings.output_extension,
            &settings.output_suffix,
            &settings.replace_strategy,
            settings.output_root.as_deref(),
        )
        .await
    }

    /// Set UI preference
    pub async fn set_preference(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO ui_preferences (key, value, updated_at) VALUES (?, ?, CURRENT_TIMESTAMP)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = CURRENT_TIMESTAMP",
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get UI preference
    pub async fn get_preference(&self, key: &str) -> Result<Option<String>> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value FROM ui_preferences WHERE key = ?")
                .bind(key)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|r| r.0))
    }

    pub async fn delete_preference(&self, key: &str) -> Result<()> {
        sqlx::query("DELETE FROM ui_preferences WHERE key = ?")
            .bind(key)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db_path(prefix: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("{prefix}_{}.db", rand::random::<u64>()));
        path
    }

    fn sample_profile(name: &str) -> NewLibraryProfile {
        NewLibraryProfile {
            name: name.to_string(),
            preset: "balanced".to_string(),
            codec: "av1".to_string(),
            quality_profile: "balanced".to_string(),
            hdr_mode: "preserve".to_string(),
            audio_mode: "copy".to_string(),
            crf_override: None,
            notes: None,
        }
    }

    #[tokio::test]
    async fn profile_lookup_treats_percent_and_underscore_as_literals() -> anyhow::Result<()> {
        let db_path = temp_db_path("alchemist_profile_lookup_literals");
        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;

        let underscore_profile = db.create_profile(sample_profile("underscore")).await?;
        let percent_profile = db.create_profile(sample_profile("percent")).await?;

        let underscore_watch = db.add_watch_dir("/media/TV_4K", true).await?;
        db.assign_profile_to_watch_dir(underscore_watch.id, Some(underscore_profile))
            .await?;

        let percent_watch = db.add_watch_dir("/media/Movies%20", true).await?;
        db.assign_profile_to_watch_dir(percent_watch.id, Some(percent_profile))
            .await?;

        assert_eq!(
            db.get_profile_for_path("/media/TV_4K/show/file.mkv")
                .await?
                .map(|profile| profile.name),
            Some("underscore".to_string())
        );
        assert_eq!(
            db.get_profile_for_path("/media/TVA4K/show/file.mkv")
                .await?
                .map(|profile| profile.name),
            None
        );
        assert_eq!(
            db.get_profile_for_path("/media/Movies%20/title/file.mkv")
                .await?
                .map(|profile| profile.name),
            Some("percent".to_string())
        );
        assert_eq!(
            db.get_profile_for_path("/media/MoviesABCD/title/file.mkv")
                .await?
                .map(|profile| profile.name),
            None
        );

        db.pool.close().await;
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }

    #[tokio::test]
    async fn profile_lookup_prefers_longest_literal_matching_watch_dir() -> anyhow::Result<()> {
        let db_path = temp_db_path("alchemist_profile_lookup_longest");
        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;

        let base_profile = db.create_profile(sample_profile("base")).await?;
        let nested_profile = db.create_profile(sample_profile("nested")).await?;

        let base_watch = db.add_watch_dir("/media", true).await?;
        db.assign_profile_to_watch_dir(base_watch.id, Some(base_profile))
            .await?;

        let nested_watch = db.add_watch_dir("/media/TV_4K", true).await?;
        db.assign_profile_to_watch_dir(nested_watch.id, Some(nested_profile))
            .await?;

        assert_eq!(
            db.get_profile_for_path("/media/TV_4K/show/file.mkv")
                .await?
                .map(|profile| profile.name),
            Some("nested".to_string())
        );

        db.pool.close().await;
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }
}
