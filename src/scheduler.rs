use crate::db::Db;
use crate::Agent;
use chrono::{Datelike, Local, Timelike};
use std::sync::Arc;
use tokio::time::Duration;
use tracing::{error, info, warn};

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
        let current_minutes = now.hour() * 60 + now.minute();
        let current_day = now.weekday().num_days_from_sunday() as i32; // 0=Sun, 6=Sat

        let mut in_window = false;

        for window in enabled_windows {
            // Parse days
            let days: Vec<i32> = serde_json::from_str(&window.days_of_week).unwrap_or_default();
            if !days.contains(&current_day) {
                continue;
            }

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

            // Check time
            // Handle cross-day windows (e.g. 23:00 to 02:00)
            if start_minutes <= end_minutes {
                // Normal window
                if current_minutes >= start_minutes && current_minutes < end_minutes {
                    in_window = true;
                    break;
                }
            } else {
                // Split window
                if current_minutes >= start_minutes || current_minutes < end_minutes {
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
