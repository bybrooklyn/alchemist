//! Notification system for job events
//!
//! Supports generic webhooks and Discord webhooks.

use crate::config::NotificationsConfig;
use crate::db::Job;
use crate::error::{AlchemistError, Result};
use serde::Serialize;
use tracing::debug;

/// Notification service for sending alerts
pub struct NotificationService {
    config: NotificationsConfig,
}

impl NotificationService {
    pub fn new(config: NotificationsConfig) -> Self {
        Self { config }
    }

    /// Check if notifications are enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
            && (self.config.webhook_url.is_some() || self.config.discord_webhook.is_some())
    }

    /// Send notification for job completion
    pub async fn notify_job_complete(&self, job: &Job, stats: Option<&str>) -> Result<()> {
        if !self.config.enabled || !self.config.notify_on_complete {
            return Ok(());
        }

        let message = format!(
            "✅ **Job #{} Completed**\n📁 {}\n{}",
            job.id,
            job.input_path,
            stats.unwrap_or("")
        );

        self.send_all(&message, "Job Completed", 0x00FF00).await
    }

    /// Send notification for job failure
    pub async fn notify_job_failed(&self, job: &Job, error: &str) -> Result<()> {
        if !self.config.enabled || !self.config.notify_on_failure {
            return Ok(());
        }

        let message = format!(
            "❌ **Job #{} Failed**\n📁 {}\n⚠️ Error: {}",
            job.id, job.input_path, error
        );

        self.send_all(&message, "Job Failed", 0xFF0000).await
    }

    /// Send to all configured endpoints
    async fn send_all(&self, message: &str, title: &str, color: u32) -> Result<()> {
        let mut errors = Vec::new();

        // Send to generic webhook
        if let Some(ref url) = self.config.webhook_url {
            if let Err(e) = self.send_webhook(url, message).await {
                errors.push(format!("Webhook: {}", e));
            }
        }

        // Send to Discord
        if let Some(ref url) = self.config.discord_webhook {
            if let Err(e) = self.send_discord(url, title, message, color).await {
                errors.push(format!("Discord: {}", e));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(AlchemistError::Notification(errors.join(", ")))
        }
    }

    /// Send to a generic webhook (POST with JSON body)
    async fn send_webhook(&self, url: &str, message: &str) -> Result<()> {
        #[derive(Serialize)]
        struct WebhookPayload<'a> {
            message: &'a str,
            source: &'a str,
            timestamp: String,
        }

        let payload = WebhookPayload {
            message,
            source: "alchemist",
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        let client = reqwest::Client::new();
        let response = client
            .post(url)
            .json(&payload)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| AlchemistError::Notification(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            return Err(AlchemistError::Notification(format!(
                "Webhook returned {}",
                response.status()
            )));
        }

        debug!("Webhook notification sent successfully");
        Ok(())
    }

    /// Send to Discord webhook with embed
    async fn send_discord(&self, url: &str, title: &str, message: &str, color: u32) -> Result<()> {
        #[derive(Serialize)]
        struct DiscordEmbed<'a> {
            title: &'a str,
            description: &'a str,
            color: u32,
        }

        #[derive(Serialize)]
        struct DiscordPayload<'a> {
            username: &'a str,
            embeds: Vec<DiscordEmbed<'a>>,
        }

        let payload = DiscordPayload {
            username: "Alchemist",
            embeds: vec![DiscordEmbed {
                title,
                description: message,
                color,
            }],
        };

        let client = reqwest::Client::new();
        let response = client
            .post(url)
            .json(&payload)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| AlchemistError::Notification(format!("HTTP error: {}", e)))?;

        if !response.status().is_success() {
            return Err(AlchemistError::Notification(format!(
                "Discord returned {}",
                response.status()
            )));
        }

        debug!("Discord notification sent successfully");
        Ok(())
    }
}

/// Helper to format job stats for notifications
pub fn format_job_stats(
    input_size_mb: u64,
    output_size_mb: u64,
    reduction_pct: f64,
    duration_secs: f64,
) -> String {
    format!(
        "📊 {} MB → {} MB ({:.1}% reduction) in {:.1}s",
        input_size_mb, output_size_mb, reduction_pct, duration_secs
    )
}
