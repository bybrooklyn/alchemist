use crate::config::Config;
use crate::db::{AlchemistEvent, Db, NotificationTarget};
use crate::explanations::Explanation;
use reqwest::{Client, Url, redirect::Policy};
use serde_json::json;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::lookup_host;
use tokio::sync::{RwLock, broadcast};
use tracing::{error, warn};

#[derive(Clone)]
pub struct NotificationManager {
    db: Db,
    config: Arc<RwLock<Config>>,
}

impl NotificationManager {
    pub fn new(db: Db, config: Arc<RwLock<Config>>) -> Self {
        Self { db, config }
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

        let allow_local = self
            .config
            .read()
            .await
            .notifications
            .allow_local_notifications;

        if !allow_local && host.eq_ignore_ascii_case("localhost") {
            return Err("localhost is not allowed as a notification endpoint".into());
        }

        let addr = format!("{}:{}", host, port);
        let ips = tokio::time::timeout(Duration::from_secs(3), lookup_host(&addr)).await??;

        let target_ip = if allow_local {
            // When local notifications are allowed, accept any resolved IP
            ips.into_iter()
                .map(|a| a.ip())
                .next()
                .ok_or("no IP address found for notification endpoint")?
        } else {
            // When local notifications are blocked, only use public IPs
            ips.into_iter()
                .map(|a| a.ip())
                .find(|ip| !is_private_ip(*ip))
                .ok_or("no public IP address found for notification endpoint")?
        };

        // Pin the request to the validated IP to prevent DNS rebinding
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .redirect(Policy::none())
            .resolve(host, std::net::SocketAddr::new(target_ip, port))
            .build()?;

        let (decision_explanation, failure_explanation) = match event {
            AlchemistEvent::JobStateChanged { job_id, status } => {
                let decision_explanation = self
                    .db
                    .get_job_decision_explanation(*job_id)
                    .await
                    .ok()
                    .flatten();
                let failure_explanation = if *status == crate::db::JobState::Failed {
                    self.db
                        .get_job_failure_explanation(*job_id)
                        .await
                        .ok()
                        .flatten()
                } else {
                    None
                };
                (decision_explanation, failure_explanation)
            }
            _ => (None, None),
        };

        match target.target_type.as_str() {
            "discord" => {
                self.send_discord_with_client(
                    &client,
                    target,
                    event,
                    status,
                    decision_explanation.as_ref(),
                    failure_explanation.as_ref(),
                )
                .await
            }
            "gotify" => {
                self.send_gotify_with_client(
                    &client,
                    target,
                    event,
                    status,
                    decision_explanation.as_ref(),
                    failure_explanation.as_ref(),
                )
                .await
            }
            "webhook" => {
                self.send_webhook_with_client(
                    &client,
                    target,
                    event,
                    status,
                    decision_explanation.as_ref(),
                    failure_explanation.as_ref(),
                )
                .await
            }
            _ => Ok(()),
        }
    }

    fn notification_message(
        &self,
        job_id: i64,
        status: &str,
        decision_explanation: Option<&Explanation>,
        failure_explanation: Option<&Explanation>,
    ) -> String {
        let explanation = failure_explanation.or(decision_explanation);
        if let Some(explanation) = explanation {
            let mut message = format!("Job #{} {} — {}", job_id, status, explanation.summary);
            if !explanation.detail.is_empty() {
                message.push_str(&format!("\n{}", explanation.detail));
            }
            if let Some(guidance) = &explanation.operator_guidance {
                message.push_str(&format!("\nNext step: {}", guidance));
            }
            return message;
        }

        format!("Job #{} is now {}", job_id, status)
    }

    async fn send_discord_with_client(
        &self,
        client: &Client,
        target: &NotificationTarget,
        event: &AlchemistEvent,
        status: &str,
        decision_explanation: Option<&Explanation>,
        failure_explanation: Option<&Explanation>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let color = match status {
            "completed" => 0x00FF00,             // Green
            "failed" => 0xFF0000,                // Red
            "queued" => 0xF1C40F,                // Yellow
            "encoding" | "remuxing" => 0x3498DB, // Blue
            _ => 0x95A5A6,                       // Gray
        };

        let message = match event {
            AlchemistEvent::JobStateChanged { job_id, status } => self.notification_message(
                *job_id,
                &status.to_string(),
                decision_explanation,
                failure_explanation,
            ),
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
        decision_explanation: Option<&Explanation>,
        failure_explanation: Option<&Explanation>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let message = match event {
            AlchemistEvent::JobStateChanged { job_id, status } => self.notification_message(
                *job_id,
                &status.to_string(),
                decision_explanation,
                failure_explanation,
            ),
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
        decision_explanation: Option<&Explanation>,
        failure_explanation: Option<&Explanation>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let message = match event {
            AlchemistEvent::JobStateChanged { job_id, status } => self.notification_message(
                *job_id,
                &status.to_string(),
                decision_explanation,
                failure_explanation,
            ),
            _ => "Event occurred".to_string(),
        };

        let body = json!({
            "event": "job_update",
            "status": status,
            "message": message,
            "data": event,
            "decision_explanation": decision_explanation,
            "failure_explanation": failure_explanation,
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
    use crate::db::JobState;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn test_webhook_errors_on_non_success()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut db_path = std::env::temp_dir();
        let token: u64 = rand::random();
        db_path.push(format!("alchemist_notifications_test_{}.db", token));

        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;
        let mut test_config = crate::config::Config::default();
        test_config.notifications.allow_local_notifications = true;
        let config = Arc::new(RwLock::new(test_config));
        let manager = NotificationManager::new(db, config);

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

    #[tokio::test]
    async fn webhook_payload_includes_structured_explanations()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut db_path = std::env::temp_dir();
        let token: u64 = rand::random();
        db_path.push(format!("alchemist_notifications_payload_test_{}.db", token));

        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;
        let _ = db
            .enqueue_job(
                std::path::Path::new("notify-input.mkv"),
                std::path::Path::new("notify-output.mkv"),
                std::time::SystemTime::UNIX_EPOCH,
            )
            .await?;
        let job = db
            .get_job_by_input_path("notify-input.mkv")
            .await?
            .ok_or("missing job")?;
        db.update_job_status(job.id, JobState::Failed).await?;
        db.add_decision(job.id, "skip", "planning_failed|error=boom")
            .await?;
        db.upsert_job_failure_explanation(
            job.id,
            &crate::explanations::failure_from_summary("Unknown encoder 'missing_encoder'"),
        )
        .await?;

        let mut test_config = crate::config::Config::default();
        test_config.notifications.allow_local_notifications = true;
        let config = Arc::new(RwLock::new(test_config));
        let manager = NotificationManager::new(db, config);

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;

        let body_task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let read = socket.read(&mut chunk).await.expect("read");
                if read == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..read]);
                if buf.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            let response = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n";
            socket.write_all(response.as_bytes()).await.expect("write");
            String::from_utf8_lossy(&buf).to_string()
        });

        let target = NotificationTarget {
            id: 0,
            name: "test".to_string(),
            target_type: "webhook".to_string(),
            endpoint_url: format!("http://{}", addr),
            auth_token: None,
            events: "[\"failed\"]".to_string(),
            enabled: true,
            created_at: chrono::Utc::now(),
        };
        let event = AlchemistEvent::JobStateChanged {
            job_id: job.id,
            status: JobState::Failed,
        };

        manager.send(&target, &event, "failed").await?;
        let request = body_task.await?;
        let body = request
            .split("\r\n\r\n")
            .nth(1)
            .ok_or("missing request body")?;
        let payload: serde_json::Value = serde_json::from_str(body)?;
        assert_eq!(
            payload["failure_explanation"]["code"].as_str(),
            Some("encoder_unavailable")
        );
        assert_eq!(
            payload["decision_explanation"]["code"].as_str(),
            Some("planning_failed")
        );

        drop(manager);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }
}
