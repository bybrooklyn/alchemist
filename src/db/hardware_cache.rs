use crate::error::Result;
use crate::system::hardware::{HardwareInfo, HardwareProbeLog};

use super::Db;

#[derive(Debug, Clone)]
pub struct HardwareDetectionCacheEntry {
    pub hardware_info: HardwareInfo,
    pub probe_log: HardwareProbeLog,
    pub detected_at: String,
}

impl Db {
    pub async fn get_hardware_detection_cache(
        &self,
        cache_key: &str,
    ) -> Result<Option<HardwareDetectionCacheEntry>> {
        let row = sqlx::query_as::<_, (String, String, String)>(
            "SELECT hardware_info_json, probe_log_json, detected_at
             FROM hardware_detection_cache
             WHERE id = 1 AND cache_key = ?",
        )
        .bind(cache_key)
        .fetch_optional(&self.pool)
        .await?;

        let Some((hardware_info_json, probe_log_json, detected_at)) = row else {
            return Ok(None);
        };

        let hardware_info = match serde_json::from_str(&hardware_info_json) {
            Ok(value) => value,
            Err(err) => {
                tracing::warn!("Ignoring invalid hardware detection cache payload: {err}");
                return Ok(None);
            }
        };
        let probe_log = match serde_json::from_str(&probe_log_json) {
            Ok(value) => value,
            Err(err) => {
                tracing::warn!("Ignoring invalid hardware probe cache payload: {err}");
                return Ok(None);
            }
        };

        Ok(Some(HardwareDetectionCacheEntry {
            hardware_info,
            probe_log,
            detected_at,
        }))
    }

    pub async fn upsert_hardware_detection_cache(
        &self,
        cache_key: &str,
        fingerprint_json: &str,
        hardware_info: &HardwareInfo,
        probe_log: &HardwareProbeLog,
    ) -> Result<()> {
        let hardware_info_json = serde_json::to_string(hardware_info)
            .map_err(|err| crate::error::AlchemistError::Unknown(err.to_string()))?;
        let probe_log_json = serde_json::to_string(probe_log)
            .map_err(|err| crate::error::AlchemistError::Unknown(err.to_string()))?;

        sqlx::query(
            "INSERT INTO hardware_detection_cache
                (id, cache_key, fingerprint_json, hardware_info_json, probe_log_json, detected_at, updated_at)
             VALUES (1, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
             ON CONFLICT(id) DO UPDATE SET
                cache_key = excluded.cache_key,
                fingerprint_json = excluded.fingerprint_json,
                hardware_info_json = excluded.hardware_info_json,
                probe_log_json = excluded.probe_log_json,
                detected_at = CURRENT_TIMESTAMP,
                updated_at = CURRENT_TIMESTAMP",
        )
        .bind(cache_key)
        .bind(fingerprint_json)
        .bind(hardware_info_json)
        .bind(probe_log_json)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Db;
    use crate::system::hardware::{
        HardwareDetectionCacheFingerprint, HardwareInfo, HardwareProbeLog, ProbeSummary, Vendor,
    };
    use std::fs;

    fn temp_db_path(name: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("alchemist_{name}_{}.db", rand::random::<u64>()));
        path
    }

    fn test_fingerprint(device_path: Option<&str>) -> HardwareDetectionCacheFingerprint {
        HardwareDetectionCacheFingerprint {
            os: "linux".to_string(),
            arch: "x86_64".to_string(),
            ffmpeg_version: "ffmpeg 7".to_string(),
            ffprobe_version: "ffprobe 7".to_string(),
            preferred_vendor: Some("intel".to_string()),
            device_path: device_path.map(str::to_string),
            allow_cpu_fallback: true,
            allow_cpu_encoding: true,
            detection_version: 1,
        }
    }

    fn test_hardware() -> HardwareInfo {
        HardwareInfo {
            vendor: Vendor::Intel,
            device_path: Some("/dev/dri/renderD128".to_string()),
            supported_codecs: vec!["h264".to_string(), "hevc".to_string()],
            backends: Vec::new(),
            detection_notes: vec!["cached".to_string()],
            selection_reason: "test".to_string(),
            probe_summary: ProbeSummary {
                attempted: 2,
                succeeded: 2,
                failed: 0,
            },
        }
    }

    #[tokio::test]
    async fn hardware_detection_cache_round_trips_and_keys_by_fingerprint() -> anyhow::Result<()> {
        let db_path = temp_db_path("hardware_cache_round_trip");
        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;
        let fingerprint = test_fingerprint(Some("/dev/dri/renderD128"));
        let cache_key = fingerprint.cache_key()?;
        let fingerprint_json = fingerprint.to_cache_json()?;
        let hardware = test_hardware();
        let probe_log = HardwareProbeLog::default();

        assert!(db.get_hardware_detection_cache(&cache_key).await?.is_none());

        db.upsert_hardware_detection_cache(&cache_key, &fingerprint_json, &hardware, &probe_log)
            .await?;

        let cached = db
            .get_hardware_detection_cache(&cache_key)
            .await?
            .ok_or_else(|| anyhow::anyhow!("missing hardware cache"))?;
        assert_eq!(cached.hardware_info.vendor, Vendor::Intel);
        assert_eq!(
            cached.hardware_info.device_path.as_deref(),
            Some("/dev/dri/renderD128")
        );
        assert_eq!(
            cached.hardware_info.supported_codecs,
            vec!["h264".to_string(), "hevc".to_string()]
        );
        assert!(!cached.detected_at.is_empty());

        let changed_key = test_fingerprint(Some("/dev/dri/renderD129")).cache_key()?;
        assert!(
            db.get_hardware_detection_cache(&changed_key)
                .await?
                .is_none()
        );

        drop(db);
        let _ = fs::remove_file(db_path);
        Ok(())
    }
}
