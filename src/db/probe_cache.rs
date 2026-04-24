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
        sqlx::query(
            "INSERT INTO media_probe_cache
                (input_path, mtime_ns, size_bytes, probe_version, analysis_json, updated_at, last_accessed_at)
             VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
             ON CONFLICT(input_path, mtime_ns, size_bytes, probe_version) DO UPDATE SET
                analysis_json = excluded.analysis_json,
                updated_at = CURRENT_TIMESTAMP,
                last_accessed_at = CURRENT_TIMESTAMP",
        )
        .bind(input_path)
        .bind(mtime_ns)
        .bind(size_bytes)
        .bind(probe_version)
        .bind(analysis_json)
        .execute(&self.pool)
        .await?;

        Ok(())
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
