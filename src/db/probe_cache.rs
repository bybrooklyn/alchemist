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
        let cached_json: Option<String> = sqlx::query_scalar(
            "SELECT analysis_json
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

        if cached_json.is_some() {
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
        }

        Ok(cached_json)
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
        let mut like_pattern = normalized.clone();
        like_pattern.push('%');

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
}
