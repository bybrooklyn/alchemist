use crate::Agent;
use crate::db::Db;
use chrono::{Datelike, Local, Timelike};
use std::sync::Arc;
use tokio::sync::Notify;
use tokio::time::Duration;
use tracing::{error, info, warn};

pub struct Scheduler {
    db: Arc<Db>,
    agent: Arc<Agent>,
    notify: Arc<Notify>,
}

#[derive(Clone)]
pub struct SchedulerHandle {
    notify: Arc<Notify>,
}

impl Scheduler {
    pub fn new(db: Arc<Db>, agent: Arc<Agent>) -> Self {
        Self {
            db,
            agent,
            notify: Arc::new(Notify::new()),
        }
    }

    pub fn handle(&self) -> SchedulerHandle {
        SchedulerHandle {
            notify: self.notify.clone(),
        }
    }

    pub fn start(self) -> SchedulerHandle {
        let handle = self.handle();
        let notify = self.notify.clone();
        tokio::spawn(async move {
            info!("Scheduler started");
            loop {
                if let Err(e) = self.check_schedule().await {
                    error!("Scheduler check failed: {}", e);
                }
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(60)) => {}
                    _ = notify.notified() => {}
                }
            }
        });
        handle
    }

    async fn check_schedule(&self) -> Result<(), Box<dyn std::error::Error>> {
        let windows: Vec<crate::db::ScheduleWindow> = self.db.get_schedule_windows().await?;

        // Filter for enabled windows
        let enabled_windows: Vec<_> = windows.into_iter().filter(|w| w.enabled).collect();

        if enabled_windows.is_empty() {
            // No schedule active -> no restriction. This must clear an existing
            // scheduler pause, not just skip: leaving it set stranded the engine
            // permanently paused whenever the last window was disabled or deleted
            // while outside it. `Agent::resume()` only clears the *manual* pause,
            // and `is_paused()` ORs the two, so the UI's Resume button could not
            // recover it either — only a process restart could.
            if self.agent.is_scheduler_paused() {
                info!("No enabled schedule windows remain — clearing scheduler pause.");
                self.agent.set_scheduler_paused(false);
            }
            return Ok(());
        }

        let now = Local::now();
        let current_minutes = now.hour() * 60 + now.minute();
        let current_day = now.weekday().num_days_from_sunday() as i32; // 0=Sun, 6=Sat
        let previous_day = (current_day + 6) % 7;

        let mut in_window = false;

        for window in enabled_windows {
            // Parse days
            let days: Vec<i32> = match serde_json::from_str(&window.days_of_week) {
                Ok(v) => v,
                Err(e) => {
                    warn!(
                        "Failed to parse days_of_week for schedule window {}: {}",
                        window.id, e
                    );
                    continue;
                }
            };

            let start_minutes = match parse_schedule_minutes(&window.start_time) {
                Some(value) => value,
                None => {
                    warn!("Invalid schedule start_time '{}'", window.start_time);
                    continue;
                }
            };
            let end_minutes = match parse_schedule_minutes(&window.end_time) {
                Some(value) => value,
                None => {
                    warn!("Invalid schedule end_time '{}'", window.end_time);
                    continue;
                }
            };

            if start_minutes <= end_minutes {
                // Normal same-day window.
                if days.contains(&current_day)
                    && current_minutes >= start_minutes
                    && current_minutes < end_minutes
                {
                    in_window = true;
                    break;
                }
            } else {
                // Overnight window split across two calendar days.
                let in_late_segment =
                    days.contains(&current_day) && current_minutes >= start_minutes;
                let in_early_segment =
                    days.contains(&previous_day) && current_minutes < end_minutes;
                if in_late_segment || in_early_segment {
                    in_window = true;
                    break;
                }
            }
        }

        if in_window {
            // Allowed to run
            if self.agent.is_scheduler_paused() {
                self.agent.set_scheduler_paused(false);
            }
        } else {
            // RESTRICTED
            if !self.agent.is_scheduler_paused() {
                self.agent.set_scheduler_paused(true);
            }
        }

        Ok(())
    }
}

impl SchedulerHandle {
    pub fn trigger(&self) {
        self.notify.notify_one();
    }
}

fn parse_schedule_minutes(value: &str) -> Option<u32> {
    let trimmed = value.trim();
    let parts: Vec<&str> = trimmed.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let hour: u32 = parts[0].parse().ok()?;
    let minute: u32 = parts[1].parse().ok()?;
    if hour > 23 || minute > 59 {
        return None;
    }
    Some(hour * 60 + minute)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Transcoder;
    use crate::system::hardware::{HardwareInfo, HardwareState, ProbeSummary, Vendor};
    use tokio::sync::RwLock;

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    async fn test_scheduler()
    -> std::result::Result<(Scheduler, Arc<Agent>, std::path::PathBuf), Box<dyn std::error::Error>>
    {
        let db_path = std::env::temp_dir().join(format!(
            "alchemist_scheduler_test_{}.db",
            rand::random::<u64>()
        ));
        let db = Arc::new(Db::new(db_path.to_string_lossy().as_ref()).await?);
        let (jobs_tx, _) = tokio::sync::broadcast::channel(16);
        let (config_tx, _) = tokio::sync::broadcast::channel(16);
        let (system_tx, _) = tokio::sync::broadcast::channel(16);
        let agent = Arc::new(
            Agent::new(
                db.clone(),
                Arc::new(Transcoder::new()),
                Arc::new(RwLock::new(crate::config::Config::default())),
                HardwareState::new(Some(HardwareInfo {
                    vendor: Vendor::Cpu,
                    device_path: None,
                    supported_codecs: Vec::new(),
                    backends: Vec::new(),
                    detection_notes: Vec::new(),
                    selection_reason: String::new(),
                    probe_summary: ProbeSummary::default(),
                })),
                Arc::new(crate::db::EventChannels {
                    jobs: jobs_tx,
                    config: config_tx,
                    system: system_tx,
                }),
                true,
            )
            .await,
        );

        Ok((Scheduler::new(db, agent.clone()), agent, db_path))
    }

    /// Removing (or disabling) the last schedule window must release the
    /// scheduler pause. It used to early-return and leave `scheduler_paused`
    /// set forever: `Agent::resume()` only clears the *manual* pause and
    /// `is_paused()` ORs the two, so the UI's Resume button could not recover
    /// the engine — only restarting the process could.
    #[tokio::test]
    async fn no_enabled_windows_clears_an_existing_scheduler_pause() -> TestResult {
        let (scheduler, agent, db_path) = test_scheduler().await?;

        agent.set_scheduler_paused(true);
        assert!(agent.is_scheduler_paused());

        scheduler.check_schedule().await?;

        assert!(
            !agent.is_scheduler_paused(),
            "engine left paused by a schedule that no longer exists"
        );

        drop(scheduler);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }
}
