use crate::db::{AlchemistEvent, Db, NotificationTarget};
use reqwest::{Client, Url, redirect::Policy};
use serde_json::json;
use std::net::IpAddr;
use std::time::Duration;
use tokio::net::lookup_host;
use tokio::sync::broadcast;
use tracing::{error, warn};

#[derive(Clone)]
pub struct NotificationManager {
    db: Db,
}

impl NotificationManager {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    pub fn start_listener(&self, mut rx: broadcast::Receiver<AlchemistEvent>) {
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
            let allowed: Vec<String> = match serde_json::from_str(&target.events) {
                Ok(v) => v,
                Err(e) => {
                    warn!(
                        "Failed to parse events for notification target '{}': {}",
                        target.name, e
                    );
                    Vec::new()
                }
            };

            if allowed.contains(&status) {
                let manager = self.clone();
                let event_clone = event.clone();
                let status_clone = status.clone();
                tokio::spawn(async move {
                    if let Err(e) = manager.send(&target, &event_clone, &status_clone).await {
                        error!(
                            "Failed to send notification to target '{}': {}",
                            target.name, e
                        );
                    }
                });
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
        let url = Url::parse(&target.endpoint_url)?;
        let host = url
            .host_str()
            .ok_or("notification endpoint host is missing")?;
        let port = url.port_or_known_default().ok_or("invalid port")?;

        if host.eq_ignore_ascii_case("localhost") {
            return Err("localhost is not allowed as a notification endpoint".into());
        }

        let addr = format!("{}:{}", host, port);
        let ips = tokio::time::timeout(Duration::from_secs(3), lookup_host(&addr)).await??;
        let target_ip = ips
            .into_iter()
            .map(|a| a.ip())
            .find(|ip| !is_private_ip(*ip))
            .ok_or("no public IP address found for notification endpoint")?;

        // Pin the request to the validated IP to prevent DNS rebinding
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .redirect(Policy::none())
            .resolve(host, std::net::SocketAddr::new(target_ip, port))
            .build()?;

        match target.target_type.as_str() {
            "discord" => {
                self.send_discord_with_client(&client, target, event, status)
                    .await
            }
            "gotify" => {
                self.send_gotify_with_client(&client, target, event, status)
                    .await
            }
            "webhook" => {
                self.send_webhook_with_client(&client, target, event, status)
                    .await
            }
            _ => Ok(()),
        }
    }

    async fn send_discord_with_client(
        &self,
        client: &Client,
        target: &NotificationTarget,
        event: &AlchemistEvent,
        status: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let color = match status {
            "completed" => 0x00FF00,             // Green
            "failed" => 0xFF0000,                // Red
            "queued" => 0xF1C40F,                // Yellow
            "encoding" | "remuxing" => 0x3498DB, // Blue
            _ => 0x95A5A6,                       // Gray
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

        client
            .post(&target.endpoint_url)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn send_gotify_with_client(
        &self,
        client: &Client,
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

        let mut req = client.post(&target.endpoint_url).json(&json!({
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

    async fn send_webhook_with_client(
        &self,
        client: &Client,
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

        let mut req = client.post(&target.endpoint_url).json(&body);
        if let Some(token) = &target.auth_token {
            req = req.bearer_auth(token);
        }

        req.send().await?.error_for_status()?;
        Ok(())
    }
}

async fn _unused_ensure_public_endpoint(raw: &str) -> Result<(), Box<dyn std::error::Error>> {
    let url = Url::parse(raw)?;
    let host = match url.host_str() {
        Some(value) => value,
        None => return Err("notification endpoint host is missing".into()),
    };
    if host.eq_ignore_ascii_case("localhost") {
        return Err("notification endpoint host is not allowed".into());
    }

    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_private_ip(ip) {
            return Err("notification endpoint host is not allowed".into());
        }
        return Ok(());
    }

    let port = match url.port_or_known_default() {
        Some(value) => value,
        None => return Err("notification endpoint port is missing".into()),
    };
    let host_port = format!("{}:{}", host, port);
    let mut resolved = false;
    let addrs = tokio::time::timeout(Duration::from_secs(3), lookup_host(host_port)).await??;
    for addr in addrs {
        resolved = true;
        if is_private_ip(addr.ip()) {
            return Err("notification endpoint host is not allowed".into());
        }
    }
    if !resolved {
        return Err("notification endpoint host could not be resolved".into());
    }

    Ok(())
}

fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_multicast()
                || v4.is_unspecified()
                || v4.is_broadcast()
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
                || v6.is_multicast()
                || v6.is_unspecified()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn test_webhook_errors_on_non_success()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut db_path = std::env::temp_dir();
        let token: u64 = rand::random();
        db_path.push(format!("alchemist_notifications_test_{}.db", token));

        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;
        let manager = NotificationManager::new(db);

        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                // Some CI/sandbox environments deny opening loopback sockets.
                return Ok(());
            }
            Err(err) => return Err(err.into()),
        };
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

        let result = manager.send(&target, &event, "failed").await;
        assert!(result.is_err());

        drop(manager);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }
}
