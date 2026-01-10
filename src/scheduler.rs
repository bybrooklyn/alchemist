use crate::db::Db;
use crate::Agent;
use chrono::{Datelike, Local, Timelike};
use std::sync::Arc;
use tokio::time::Duration;
use tracing::{error, info};

pub struct Scheduler {
    db: Arc<Db>,
    agent: Arc<Agent>,
}

impl Scheduler {
    pub fn new(db: Arc<Db>, agent: Arc<Agent>) -> Self {
        Self { db, agent }
    }

    pub fn start(self) {
        tokio::spawn(async move {
            info!("Scheduler started");
            loop {
                if let Err(e) = self.check_schedule().await {
                    error!("Scheduler check failed: {}", e);
                }
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        });
    }

    async fn check_schedule(&self) -> Result<(), Box<dyn std::error::Error>> {
        let windows: Vec<crate::db::ScheduleWindow> = self.db.get_schedule_windows().await?;

        // Filter for enabled windows
        let enabled_windows: Vec<_> = windows.into_iter().filter(|w| w.enabled).collect();

        if enabled_windows.is_empty() {
            // No schedule active -> Always Run
            if self.agent.is_scheduler_paused() {
                self.agent.set_scheduler_paused(false);
            }
            return Ok(());
        }

        let now = Local::now();
        let current_time_str = format!("{:02}:{:02}", now.hour(), now.minute());
        let current_day = now.weekday().num_days_from_sunday() as i32; // 0=Sun, 6=Sat

        let mut in_window = false;

        for window in enabled_windows {
            // Parse days
            let days: Vec<i32> = serde_json::from_str(&window.days_of_week).unwrap_or_default();
            if !days.contains(&current_day) {
                continue;
            }

            // Check time
            // Handle cross-day windows (e.g. 23:00 to 02:00)
            if window.start_time <= window.end_time {
                // Normal window
                if current_time_str >= window.start_time && current_time_str < window.end_time {
                    in_window = true;
                    break;
                }
            } else {
                // Split window
                if current_time_str >= window.start_time || current_time_str < window.end_time {
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
