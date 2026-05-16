use crate::error::Result;

use super::Db;

impl Db {
    pub async fn get_media_probe_cache(
        &self,
        input_path: &str,
        mtime_ns: i64,
        size_bytes: i64,
        probe_version: &str,
    ) -> Result<Option<String>> {
        self.get_media_probe_cache_with_file_id(
            input_path,
            mtime_ns,
            size_bytes,
            probe_version,
            None,
        )
        .await
    }

    /// PERF-3 cache read: looks up a cached probe by `(input_path, mtime_ns,
    /// size_bytes, probe_version)` and, when both the stored row and the
    /// caller supply a `file_id` identity hint, verifies they match before
    /// trusting the cache.
    ///
    /// Identity rules:
    /// - both `Some` and equal → cache hit
    /// - both `Some` and different → miss (the on-disk object was replaced
    ///   even though path/size/mtime collide; re-probe)
    /// - exactly one side `Some` → miss (cannot prove identity with only
    ///   half the information; re-probe rather than trust)
    /// - both `None` → hit (legacy rows / platforms without a file id)
    pub async fn get_media_probe_cache_with_file_id(
        &self,
        input_path: &str,
        mtime_ns: i64,
        size_bytes: i64,
        probe_version: &str,
        file_id: Option<&str>,
    ) -> Result<Option<String>> {
        let row: Option<(String, Option<String>)> = sqlx::query_as(
            "SELECT analysis_json, file_id
             FROM media_probe_cache
             WHERE input_path = ?
               AND mtime_ns = ?
               AND size_bytes = ?
               AND probe_version = ?",
        )
        .bind(input_path)
        .bind(mtime_ns)
        .bind(size_bytes)
        .bind(probe_version)
        .fetch_optional(&self.pool)
        .await?;

        let Some((cached_json, stored_file_id)) = row else {
            return Ok(None);
        };

        let identity_ok = match (stored_file_id.as_deref(), file_id) {
            (Some(stored), Some(current)) => stored == current,
            (None, None) => true,
            // Half-known identity — don't trust a metadata-only match.
            _ => false,
        };
        if !identity_ok {
            return Ok(None);
        }

        let _ = sqlx::query(
            "UPDATE media_probe_cache
             SET last_accessed_at = CURRENT_TIMESTAMP
             WHERE input_path = ?
               AND mtime_ns = ?
               AND size_bytes = ?
               AND probe_version = ?",
        )
        .bind(input_path)
        .bind(mtime_ns)
        .bind(size_bytes)
        .bind(probe_version)
        .execute(&self.pool)
        .await;

        Ok(Some(cached_json))
    }

    pub async fn upsert_media_probe_cache(
        &self,
        input_path: &str,
        mtime_ns: i64,
        size_bytes: i64,
        probe_version: &str,
        analysis_json: &str,
    ) -> Result<()> {
        self.upsert_media_probe_cache_with_file_id(
            input_path,
            mtime_ns,
            size_bytes,
            probe_version,
            analysis_json,
            None,
        )
        .await
    }

    /// PERF-3 extension: store the optional `file_id` (inode on Unix, volume
    /// file index on Windows) alongside the cache row. When present on both
    /// the cached row and the file being checked, callers can verify that
    /// path+size+mtime really match the same on-disk object instead of a
    /// replaced inode that happens to share metadata.
    pub async fn upsert_media_probe_cache_with_file_id(
        &self,
        input_path: &str,
        mtime_ns: i64,
        size_bytes: i64,
        probe_version: &str,
        analysis_json: &str,
        file_id: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO media_probe_cache
                (input_path, mtime_ns, size_bytes, probe_version, analysis_json, file_id, updated_at, last_accessed_at)
             VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
             ON CONFLICT(input_path, mtime_ns, size_bytes, probe_version) DO UPDATE SET
                analysis_json = excluded.analysis_json,
                file_id = excluded.file_id,
                updated_at = CURRENT_TIMESTAMP,
                last_accessed_at = CURRENT_TIMESTAMP",
        )
        .bind(input_path)
        .bind(mtime_ns)
        .bind(size_bytes)
        .bind(probe_version)
        .bind(analysis_json)
        .bind(file_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// PERF-3: drop every cached probe whose input path lives under the
    /// supplied directory prefix. Used by the `Force full scan` flow so the
    /// next analysis pass for that watch root re-probes every file from
    /// scratch instead of trusting stale signatures.
    ///
    /// The trailing separator is preserved in the LIKE match so that
    /// `/media/movies` does not also match `/media/movies-archive`.
    pub async fn clear_media_probe_cache_under(&self, path_prefix: &str) -> Result<u64> {
        let normalized = if path_prefix.ends_with('/') || path_prefix.ends_with('\\') {
            path_prefix.to_string()
        } else if path_prefix.contains('\\') && !path_prefix.contains('/') {
            format!("{}\\", path_prefix)
        } else {
            format!("{}/", path_prefix)
        };
        // The LIKE escape character is `\`. Windows paths are full of
        // backslashes and library paths can contain `%`/`_`, so every
        // wildcard-significant character in the prefix must be escaped
        // before it is bound — otherwise the match is undefined behaviour
        // (see audit P2-29). Mirrors the search escaping in db/jobs.rs.
        let escaped = normalized
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let like_pattern = format!("{}%", escaped);

        // Match exact root file *and* anything beneath it. The exact-equality
        // branch handles the edge case where a watch root is itself a file.
        let result = sqlx::query(
            "DELETE FROM media_probe_cache
             WHERE input_path = ? OR input_path LIKE ? ESCAPE '\\'",
        )
        .bind(path_prefix)
        .bind(like_pattern)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::Db;
    use std::fs;

    fn temp_db_path(name: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("alchemist_{name}_{}.db", rand::random::<u64>()));
        path
    }

    #[tokio::test]
    async fn media_probe_cache_round_trips_and_keys_by_file_metadata() -> anyhow::Result<()> {
        let db_path = temp_db_path("probe_cache_round_trip");
        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;

        let missing = db
            .get_media_probe_cache("/media/movie.mkv", 100, 200, "ffprobe 1")
            .await?;
        assert_eq!(missing, None);

        db.upsert_media_probe_cache(
            "/media/movie.mkv",
            100,
            200,
            "ffprobe 1",
            "{\"codec\":\"av1\"}",
        )
        .await?;

        let cached = db
            .get_media_probe_cache("/media/movie.mkv", 100, 200, "ffprobe 1")
            .await?;
        assert_eq!(cached.as_deref(), Some("{\"codec\":\"av1\"}"));

        let changed_size = db
            .get_media_probe_cache("/media/movie.mkv", 100, 201, "ffprobe 1")
            .await?;
        assert_eq!(changed_size, None);

        db.upsert_media_probe_cache(
            "/media/movie.mkv",
            100,
            200,
            "ffprobe 1",
            "{\"codec\":\"hevc\"}",
        )
        .await?;
        let updated = db
            .get_media_probe_cache("/media/movie.mkv", 100, 200, "ffprobe 1")
            .await?;
        assert_eq!(updated.as_deref(), Some("{\"codec\":\"hevc\"}"));

        drop(db);
        let _ = fs::remove_file(db_path);
        Ok(())
    }

    #[tokio::test]
    async fn file_id_mismatch_is_treated_as_cache_miss() -> anyhow::Result<()> {
        let db_path = temp_db_path("probe_cache_file_id");
        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;

        db.upsert_media_probe_cache_with_file_id(
            "/media/movie.mkv",
            100,
            200,
            "ffprobe 1",
            "{\"codec\":\"av1\"}",
            Some("ino:1"),
        )
        .await?;

        // Same path/size/mtime but a different on-disk object → miss.
        let replaced = db
            .get_media_probe_cache_with_file_id(
                "/media/movie.mkv",
                100,
                200,
                "ffprobe 1",
                Some("ino:2"),
            )
            .await?;
        assert_eq!(replaced, None, "differing file_id must miss");

        // Matching identity → hit.
        let same = db
            .get_media_probe_cache_with_file_id(
                "/media/movie.mkv",
                100,
                200,
                "ffprobe 1",
                Some("ino:1"),
            )
            .await?;
        assert_eq!(same.as_deref(), Some("{\"codec\":\"av1\"}"));

        // Half-known identity (stored Some, caller None) → miss.
        let half = db
            .get_media_probe_cache_with_file_id("/media/movie.mkv", 100, 200, "ffprobe 1", None)
            .await?;
        assert_eq!(half, None, "half-known identity must not be trusted");

        drop(db);
        let _ = fs::remove_file(db_path);
        Ok(())
    }

    #[tokio::test]
    async fn legacy_rows_without_file_id_still_hit() -> anyhow::Result<()> {
        let db_path = temp_db_path("probe_cache_legacy_file_id");
        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;

        // Row written by the pre-PERF-3 path: no file_id stored.
        db.upsert_media_probe_cache(
            "/media/legacy.mkv",
            1,
            2,
            "ffprobe 1",
            "{\"codec\":\"hevc\"}",
        )
        .await?;

        let hit = db
            .get_media_probe_cache_with_file_id("/media/legacy.mkv", 1, 2, "ffprobe 1", None)
            .await?;
        assert_eq!(hit.as_deref(), Some("{\"codec\":\"hevc\"}"));

        drop(db);
        let _ = fs::remove_file(db_path);
        Ok(())
    }

    #[tokio::test]
    async fn clear_under_prefix_handles_windows_and_wildcard_paths() -> anyhow::Result<()> {
        let db_path = temp_db_path("probe_cache_clear_prefix");
        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;

        // Windows-style prefix: backslashes must not be interpreted as LIKE
        // escape sequences.
        db.upsert_media_probe_cache("C:\\Users\\me\\movies\\a.mkv", 1, 2, "ffprobe 1", "{}")
            .await?;
        db.upsert_media_probe_cache("C:\\Users\\me\\music\\b.mp3", 1, 2, "ffprobe 1", "{}")
            .await?;
        let removed = db
            .clear_media_probe_cache_under("C:\\Users\\me\\movies")
            .await?;
        assert_eq!(removed, 1, "only the movies row should be cleared");
        assert!(
            db.get_media_probe_cache("C:\\Users\\me\\music\\b.mp3", 1, 2, "ffprobe 1")
                .await?
                .is_some(),
            "the music row must survive"
        );

        // Underscore in the prefix must be a literal, not a single-char
        // wildcard.
        db.upsert_media_probe_cache("/media/season_01/ep.mkv", 1, 2, "ffprobe 1", "{}")
            .await?;
        db.upsert_media_probe_cache("/media/seasonX01/ep.mkv", 1, 2, "ffprobe 1", "{}")
            .await?;
        let removed = db.clear_media_probe_cache_under("/media/season_01").await?;
        assert_eq!(
            removed, 1,
            "underscore must match literally, not as wildcard"
        );
        assert!(
            db.get_media_probe_cache("/media/seasonX01/ep.mkv", 1, 2, "ffprobe 1")
                .await?
                .is_some(),
        );

        drop(db);
        let _ = fs::remove_file(db_path);
        Ok(())
    }
}
