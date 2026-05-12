//! Persistent Prometheus metrics that accumulate across requests.
//!
//! Snapshot-style gauges (queue depth, bytes saved) still live in the request
//! handler — they are cheaper to materialize from SQL on each scrape than to
//! keep in sync as in-memory state. Everything here is a counter or histogram
//! that must survive the lifetime of the process.
//!
//! The subscriber loop in `spawn_metrics_subscriber` listens to `JobEvent`s
//! and updates these handles when jobs reach terminal states.

use crate::db::{Db, JobEvent, JobState};
use prometheus::{HistogramOpts, HistogramVec, IntCounterVec, Opts, Registry};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, warn};

/// All persistent metric handles. Cloning is cheap (Arc internally).
#[derive(Clone)]
pub struct AlchemistMetrics {
    registry: Registry,
    pub encodes_completed: IntCounterVec,
    pub encode_duration_seconds: HistogramVec,
    pub pipeline_errors: IntCounterVec,
}

impl AlchemistMetrics {
    pub fn new() -> Result<Self, prometheus::Error> {
        let registry = Registry::new();

        let encodes_completed = IntCounterVec::new(
            Opts::new(
                "alchemist_encodes_completed_total",
                "Total successful encodes since process start, labelled by output codec.",
            ),
            &["codec"],
        )?;
        let encode_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "alchemist_encode_duration_seconds",
                "Encode wall-time in seconds, labelled by output codec.",
            )
            .buckets(vec![
                10.0, 30.0, 60.0, 120.0, 300.0, 600.0, 1800.0, 3600.0, 7200.0, 14400.0,
            ]),
            &["codec"],
        )?;
        let pipeline_errors = IntCounterVec::new(
            Opts::new(
                "alchemist_pipeline_errors_total",
                "Total job failures since process start, labelled by failure code.",
            ),
            &["code"],
        )?;

        registry.register(Box::new(encodes_completed.clone()))?;
        registry.register(Box::new(encode_duration_seconds.clone()))?;
        registry.register(Box::new(pipeline_errors.clone()))?;

        Ok(Self {
            registry,
            encodes_completed,
            encode_duration_seconds,
            pipeline_errors,
        })
    }

    /// Read-only access to the registry for the scrape handler.
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    pub fn record_completion(&self, codec: &str, duration_seconds: f64) {
        self.encodes_completed.with_label_values(&[codec]).inc();
        self.encode_duration_seconds
            .with_label_values(&[codec])
            .observe(duration_seconds);
    }

    pub fn record_failure(&self, code: &str) {
        self.pipeline_errors.with_label_values(&[code]).inc();
    }
}

/// Subscribe to `JobEvent`s and update the persistent counters when jobs reach
/// `Completed` / `Failed`. Returns immediately; the loop runs as a tokio task
/// for the lifetime of the process. Drops cleanly when the broadcast sender is
/// dropped.
pub fn spawn_metrics_subscriber(
    db: Arc<Db>,
    metrics: Arc<AlchemistMetrics>,
    mut rx: broadcast::Receiver<JobEvent>,
) {
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(JobEvent::StateChanged { job_id, status }) if job_id > 0 => match status {
                    JobState::Completed => {
                        record_completion_from_db(&db, &metrics, job_id).await;
                    }
                    JobState::Failed => {
                        record_failure_from_db(&db, &metrics, job_id).await;
                    }
                    _ => {}
                },
                Ok(_) => {}
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    warn!(
                        skipped,
                        "Metrics subscriber lagged behind broadcast channel; some events were dropped"
                    );
                }
                Err(broadcast::error::RecvError::Closed) => {
                    debug!("Metrics subscriber exiting: job event channel closed");
                    return;
                }
            }
        }
    });
}

async fn record_completion_from_db(db: &Db, metrics: &AlchemistMetrics, job_id: i64) {
    match db.get_encode_completion_summary(job_id).await {
        Ok(Some(summary)) => {
            let codec = summary.codec.as_deref().unwrap_or("unknown");
            metrics.record_completion(codec, summary.encode_time_seconds.max(0.0));
        }
        Ok(None) => {
            debug!(
                job_id,
                "Completion event observed but no encode_stats row found yet; skipping metric"
            );
        }
        Err(err) => {
            warn!(job_id, error = %err, "Failed to load encode summary for metrics");
        }
    }
}

async fn record_failure_from_db(db: &Db, metrics: &AlchemistMetrics, job_id: i64) {
    match db.get_job_failure_explanation(job_id).await {
        Ok(Some(explanation)) => metrics.record_failure(&explanation.code),
        Ok(None) => metrics.record_failure("unknown"),
        Err(err) => {
            warn!(job_id, error = %err, "Failed to load failure explanation for metrics");
            metrics.record_failure("unknown");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prometheus::TextEncoder;

    #[test]
    fn record_completion_emits_counter_and_histogram() -> Result<(), prometheus::Error> {
        let metrics = AlchemistMetrics::new()?;
        metrics.record_completion("hevc", 42.0);
        metrics.record_completion("hevc", 84.0);
        metrics.record_completion("av1", 30.0);

        let mut buffer = String::new();
        let encoder = TextEncoder::new();
        let families = metrics.registry().gather();
        encoder
            .encode_utf8(&families, &mut buffer)
            .map_err(|err| prometheus::Error::Msg(err.to_string()))?;

        assert!(buffer.contains("alchemist_encodes_completed_total{codec=\"hevc\"} 2"));
        assert!(buffer.contains("alchemist_encodes_completed_total{codec=\"av1\"} 1"));
        assert!(buffer.contains("alchemist_encode_duration_seconds_count{codec=\"hevc\"} 2"));
        assert!(buffer.contains("alchemist_encode_duration_seconds_count{codec=\"av1\"} 1"));
        Ok(())
    }

    #[test]
    fn record_failure_increments_counter() -> Result<(), prometheus::Error> {
        let metrics = AlchemistMetrics::new()?;
        metrics.record_failure("source_missing");
        metrics.record_failure("source_missing");
        metrics.record_failure("encoder_unavailable");

        let mut buffer = String::new();
        let encoder = TextEncoder::new();
        encoder
            .encode_utf8(&metrics.registry().gather(), &mut buffer)
            .map_err(|err| prometheus::Error::Msg(err.to_string()))?;

        assert!(buffer.contains("alchemist_pipeline_errors_total{code=\"source_missing\"} 2"));
        assert!(buffer.contains("alchemist_pipeline_errors_total{code=\"encoder_unavailable\"} 1"));
        Ok(())
    }
}
