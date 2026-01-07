use crate::db::Job;
use leptos::*;

#[server(GetJobs, "/api")]
pub async fn get_jobs() -> Result<Vec<Job>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::db::Db;
        use axum::Extension;
        use std::sync::Arc;

        let db = use_context::<Extension<Arc<Db>>>()
            .ok_or_else(|| ServerFnError::new("DB not found"))?
            .0
            .clone();

        db.get_all_jobs()
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        unreachable!()
    }
}

#[server(GetStats, "/api")]
pub async fn get_stats() -> Result<serde_json::Value, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::db::Db;
        use axum::Extension;
        use std::sync::Arc;

        let db = use_context::<Extension<Arc<Db>>>()
            .ok_or_else(|| ServerFnError::new("DB not found"))?
            .0
            .clone();

        db.get_stats()
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        unreachable!()
    }
}

#[server(RunScan, "/api")]
pub async fn run_scan() -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::config::Config;
        use crate::Agent;
        use axum::Extension;
        use std::sync::Arc;

        let agent = use_context::<Extension<Arc<Agent>>>()
            .ok_or_else(|| ServerFnError::new("Agent not found"))?
            .0
            .clone();
        let config = use_context::<Extension<Arc<Config>>>()
            .ok_or_else(|| ServerFnError::new("Config not found"))?
            .0
            .clone();

        let dirs = config
            .scanner
            .directories
            .iter()
            .map(std::path::PathBuf::from)
            .collect();
        agent
            .scan_and_enqueue(dirs)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        unreachable!()
    }
}

#[server(CancelJob, "/api")]
pub async fn cancel_job(job_id: i64) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::Transcoder;
        use axum::Extension;
        use std::sync::Arc;

        let transcoder = use_context::<Extension<Arc<Transcoder>>>()
            .ok_or_else(|| ServerFnError::new("Transcoder not found"))?
            .0
            .clone();

        if transcoder.cancel_job(job_id) {
            Ok(())
        } else {
            Err(ServerFnError::new("Job not running or not found"))
        }
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = job_id;
        unreachable!()
    }
}

#[server(RestartJob, "/api")]
pub async fn restart_job(job_id: i64) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::db::{Db, JobState};
        use axum::Extension;
        use std::sync::Arc;

        let db = use_context::<Extension<Arc<Db>>>()
            .ok_or_else(|| ServerFnError::new("DB not found"))?
            .0
            .clone();

        db.update_job_status(job_id, JobState::Queued)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = job_id;
        unreachable!()
    }
}

#[server(RestartAllFailed, "/api")]
pub async fn restart_all_failed() -> Result<u64, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::db::{Db, JobState};
        use axum::Extension;
        use std::sync::Arc;

        let db = use_context::<Extension<Arc<Db>>>()
            .ok_or_else(|| ServerFnError::new("DB not found"))?
            .0
            .clone();

        db.batch_update_status(JobState::Failed, JobState::Queued)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        unreachable!()
    }
}

#[server(SetJobPriority, "/api")]
pub async fn set_job_priority(job_id: i64, priority: i32) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::db::Db;
        use axum::Extension;
        use std::sync::Arc;

        let db = use_context::<Extension<Arc<Db>>>()
            .ok_or_else(|| ServerFnError::new("DB not found"))?
            .0
            .clone();

        db.set_job_priority(job_id, priority)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (job_id, priority);
        unreachable!()
    }
}

#[server(GetConfig, "/api")]
pub async fn get_config() -> Result<crate::config::Config, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::config::Config;
        use axum::Extension;
        use std::sync::Arc;

        let config = use_context::<Extension<Arc<Config>>>()
            .ok_or_else(|| ServerFnError::new("Config not found"))?
            .0
            .clone();

        Ok((*config).clone())
    }
    #[cfg(not(feature = "ssr"))]
    {
        unreachable!()
    }
}
