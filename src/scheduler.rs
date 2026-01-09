//! Job scheduler for time-based processing
//!
//! Allows users to configure specific hours when transcoding should run.

use chrono::{Datelike, Local, Timelike, Weekday};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

/// Schedule configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScheduleConfig {
    /// Enable scheduling (if false, run 24/7)
    #[serde(default)]
    pub enabled: bool,

    /// Start hour (0-23)
    #[serde(default = "default_start_hour")]
    pub start_hour: u32,

    /// End hour (0-23)
    #[serde(default = "default_end_hour")]
    pub end_hour: u32,

    /// Days of week to run (empty = all days)
    #[serde(default)]
    pub days: Vec<String>,
}

fn default_start_hour() -> u32 {
    22
} // 10 PM
fn default_end_hour() -> u32 {
    6
} // 6 AM

impl ScheduleConfig {
    /// Check if we should be running right now
    pub fn should_run(&self) -> bool {
        if !self.enabled {
            return true; // If scheduling disabled, always run
        }

        let now = Local::now();
        let current_hour = now.hour();

        // Check day of week
        if !self.days.is_empty() {
            let today = match now.weekday() {
                Weekday::Mon => "mon",
                Weekday::Tue => "tue",
                Weekday::Wed => "wed",
                Weekday::Thu => "thu",
                Weekday::Fri => "fri",
                Weekday::Sat => "sat",
                Weekday::Sun => "sun",
            };

            if !self.days.iter().any(|d| d.to_lowercase() == today) {
                debug!("Scheduler: Today ({}) not in allowed days", today);
                return false;
            }
        }

        // Check time window
        let in_window = if self.start_hour <= self.end_hour {
            // Normal window (e.g., 08:00 - 17:00)
            current_hour >= self.start_hour && current_hour < self.end_hour
        } else {
            // Overnight window (e.g., 22:00 - 06:00)
            current_hour >= self.start_hour || current_hour < self.end_hour
        };

        if !in_window {
            debug!(
                "Scheduler: Current hour ({}) outside window ({}-{})",
                current_hour, self.start_hour, self.end_hour
            );
        }

        in_window
    }

    /// Format the schedule for display
    pub fn format_schedule(&self) -> String {
        if !self.enabled {
            return "24/7 (no schedule)".to_string();
        }

        let days_str = if self.days.is_empty() {
            "Every day".to_string()
        } else {
            self.days.join(", ")
        };

        format!(
            "{} from {:02}:00 to {:02}:00",
            days_str, self.start_hour, self.end_hour
        )
    }
}

/// Scheduler that can pause/resume the agent based on time
pub struct Scheduler {
    config: ScheduleConfig,
}

impl Scheduler {
    pub fn new(config: ScheduleConfig) -> Self {
        if config.enabled {
            info!("Scheduler enabled: {}", config.format_schedule());
        }
        Self { config }
    }

    pub fn update_config(&mut self, config: ScheduleConfig) {
        self.config = config;
    }

    pub fn should_run(&self) -> bool {
        self.config.should_run()
    }

    pub fn config(&self) -> &ScheduleConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disabled_scheduler() {
        let config = ScheduleConfig {
            enabled: false,
            ..Default::default()
        };
        assert!(config.should_run());
    }

    #[test]
    fn test_schedule_format() {
        let config = ScheduleConfig {
            enabled: true,
            start_hour: 22,
            end_hour: 6,
            days: vec!["mon".to_string(), "tue".to_string()],
        };
        assert_eq!(config.format_schedule(), "mon, tue from 22:00 to 06:00");
    }
}
