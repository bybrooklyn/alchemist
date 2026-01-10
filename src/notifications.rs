use crate::db::{AlchemistEvent, Db, NotificationTarget};
use reqwest::Client;
use serde_json::json;
use tokio::sync::broadcast;
use tracing::{error, warn};

#[derive(Clone)]
pub struct NotificationManager {
    db: Db,
    client: Client,
}

impl NotificationManager {
    pub fn new(db: Db) -> Self {
        Self {
            db,
            client: Client::new(),
        }
    }

    pub fn start_listener(&self, mut rx: broadcast::Receiver<AlchemistEvent>) {
        let _db = self.db.clone();
        let _client = self.client.clone();

        // Spawn a new manager instance/logic for the loop?
        // Or just move clones into the async block.
        // Self is not Clone? It has Db (Clone) and Client (Clone).
        // I can derive Clone for NotificationManager.
        // Or just move db/client.

        let manager_clone = self.clone();

        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        if let Err(e) = manager_clone.handle_event(event).await {
                            error!("Notification error: {}", e);
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        warn!("Notification listener lagged")
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    pub async fn send_test(
        &self,
        target: &NotificationTarget,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let event = AlchemistEvent::JobStateChanged {
            job_id: 0,
            status: crate::db::JobState::Completed,
        };
        self.send(target, &event, "completed").await
    }

    async fn handle_event(&self, event: AlchemistEvent) -> Result<(), Box<dyn std::error::Error>> {
        let targets = match self.db.get_notification_targets().await {
            Ok(t) => t,
            Err(e) => {
                error!("Failed to fetch notification targets: {}", e);
                return Ok(());
            }
        };

        if targets.is_empty() {
            return Ok(());
        }

        // Filter events
        let status = match &event {
            AlchemistEvent::JobStateChanged { status, .. } => status.to_string(),
            _ => return Ok(()), // Only handle job state changes for now
        };

        for target in targets {
            if !target.enabled {
                continue;
            }
            let allowed: Vec<String> = serde_json::from_str(&target.events).unwrap_or_default();

            if allowed.contains(&status) {
                self.send(&target, &event, &status).await?;
            }
        }
        Ok(())
    }

    async fn send(
        &self,
        target: &NotificationTarget,
        event: &AlchemistEvent,
        status: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match target.target_type.as_str() {
            "discord" => self.send_discord(target, event, status).await,
            "gotify" => self.send_gotify(target, event, status).await,
            "webhook" => self.send_webhook(target, event, status).await,
            _ => Ok(()),
        }
    }

    async fn send_discord(
        &self,
        target: &NotificationTarget,
        event: &AlchemistEvent,
        status: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let color = match status {
            "completed" => 0x00FF00, // Green
            "failed" => 0xFF0000,    // Red
            "queued" => 0xF1C40F,    // Yellow
            "encoding" => 0x3498DB,  // Blue
            _ => 0x95A5A6,           // Gray
        };

        let message = match event {
            AlchemistEvent::JobStateChanged { job_id, status } => {
                format!("Job #{} is now {}", job_id, status)
            }
            _ => "Event occurred".to_string(),
        };

        let body = json!({
            "embeds": [{
                "title": "Alchemist Notification",
                "description": message,
                "color": color,
                "timestamp": chrono::Utc::now().to_rfc3339()
            }]
        });

        self.client
            .post(&target.endpoint_url)
            .json(&body)
            .send()
            .await?;
        Ok(())
    }

    async fn send_gotify(
        &self,
        target: &NotificationTarget,
        event: &AlchemistEvent,
        status: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let message = match event {
            AlchemistEvent::JobStateChanged { job_id, status } => {
                format!("Job #{} is now {}", job_id, status)
            }
            _ => "Event occurred".to_string(),
        };

        let priority = match status {
            "failed" => 8,
            "completed" => 5,
            _ => 2,
        };

        let mut req = self.client.post(&target.endpoint_url).json(&json!({
            "title": "Alchemist",
            "message": message,
            "priority": priority
        }));

        if let Some(token) = &target.auth_token {
            req = req.header("X-Gotify-Key", token);
        }

        req.send().await?;
        Ok(())
    }

    async fn send_webhook(
        &self,
        target: &NotificationTarget,
        event: &AlchemistEvent,
        status: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let message = match event {
            AlchemistEvent::JobStateChanged { job_id, status } => {
                format!("Job #{} is now {}", job_id, status)
            }
            _ => "Event occurred".to_string(),
        };

        let body = json!({
            "event": "job_update",
            "status": status,
            "message": message,
            "data": event,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        let mut req = self.client.post(&target.endpoint_url).json(&body);
        if let Some(token) = &target.auth_token {
            req = req.bearer_auth(token);
        }

        req.send().await?;
        Ok(())
    }
}
