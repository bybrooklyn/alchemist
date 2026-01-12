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
            .await?
            .error_for_status()?;
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

        req.send().await?.error_for_status()?;
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

        req.send().await?.error_for_status()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn test_webhook_errors_on_non_success(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut db_path = std::env::temp_dir();
        let token: u64 = rand::random();
        db_path.push(format!("alchemist_notifications_test_{}.db", token));

        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;
        let manager = NotificationManager::new(db);

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = [0u8; 1024];
                let _ = socket.read(&mut buf).await;
                let response = "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n";
                let _ = socket.write_all(response.as_bytes()).await;
            }
        });

        let target = NotificationTarget {
            id: 0,
            name: "test".to_string(),
            target_type: "webhook".to_string(),
            endpoint_url: format!("http://{}", addr),
            auth_token: None,
            events: "[]".to_string(),
            enabled: true,
            created_at: chrono::Utc::now(),
        };
        let event = AlchemistEvent::JobStateChanged {
            job_id: 1,
            status: crate::db::JobState::Failed,
        };

        let result = manager.send_webhook(&target, &event, "failed").await;
        assert!(result.is_err());

        drop(manager);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }
}
