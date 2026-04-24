use crate::config::Config;
use crate::db::{Db, EventChannels, JobEvent, NotificationTarget, SystemEvent};
use crate::explanations::Explanation;
use chrono::Timelike;
use lettre::message::{Mailbox, Message, SinglePart, header::ContentType};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};
use reqwest::{Client, Url, redirect::Policy};
use serde::Deserialize;
use serde_json::json;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::lookup_host;
use tokio::sync::{Mutex, RwLock};
use tracing::{error, warn};

type NotificationResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
const DAILY_SUMMARY_LAST_SUCCESS_KEY: &str = "notifications.daily_summary.last_success_date";

#[derive(Clone)]
pub struct NotificationManager {
    db: Db,
    config: Arc<RwLock<Config>>,
    daily_summary_last_sent: Arc<Mutex<Option<String>>>,
}

#[derive(Debug, Deserialize)]
struct DiscordWebhookConfig {
    webhook_url: String,
}

#[derive(Debug, Deserialize)]
struct DiscordBotConfig {
    bot_token: String,
    channel_id: String,
}

#[derive(Debug, Deserialize)]
struct GotifyConfig {
    server_url: String,
    app_token: String,
}

#[derive(Debug, Deserialize)]
struct NtfyConfig {
    server_url: String,
    topic: String,
    #[serde(default)]
    access_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WebhookConfig {
    url: String,
    auth_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TelegramConfig {
    bot_token: String,
    chat_id: String,
}

#[derive(Debug, Deserialize)]
struct EmailConfig {
    smtp_host: String,
    smtp_port: u16,
    username: Option<String>,
    password: Option<String>,
    from_address: String,
    to_addresses: Vec<String>,
    security: Option<String>,
}

fn parse_target_config<T: for<'de> Deserialize<'de>>(
    target: &NotificationTarget,
) -> NotificationResult<T> {
    Ok(serde_json::from_str(&target.config_json)?)
}

fn endpoint_url_for_target(target: &NotificationTarget) -> NotificationResult<Option<String>> {
    match target.target_type.as_str() {
        "discord_webhook" => Ok(Some(
            parse_target_config::<DiscordWebhookConfig>(target)?.webhook_url,
        )),
        "gotify" => Ok(Some(
            parse_target_config::<GotifyConfig>(target)?.server_url,
        )),
        "ntfy" => Ok(Some(parse_target_config::<NtfyConfig>(target)?.server_url)),
        "webhook" => Ok(Some(parse_target_config::<WebhookConfig>(target)?.url)),
        "discord_bot" => Ok(Some("https://discord.com".to_string())),
        "telegram" => Ok(Some("https://api.telegram.org".to_string())),
        "email" => Ok(None),
        _ => Ok(None),
    }
}

/// Internal event type that unifies the events the notification system cares about.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", content = "data")]
enum NotifiableEvent {
    JobStateChanged {
        job_id: i64,
        status: crate::db::JobState,
    },
    ScanCompleted,
    EngineIdle,
}

fn event_key(event: &NotifiableEvent) -> Option<&'static str> {
    match event {
        NotifiableEvent::JobStateChanged { status, .. } => match status {
            crate::db::JobState::Queued => Some(crate::config::NOTIFICATION_EVENT_ENCODE_QUEUED),
            crate::db::JobState::Encoding | crate::db::JobState::Remuxing => {
                Some(crate::config::NOTIFICATION_EVENT_ENCODE_STARTED)
            }
            crate::db::JobState::Completed => {
                Some(crate::config::NOTIFICATION_EVENT_ENCODE_COMPLETED)
            }
            crate::db::JobState::Failed => Some(crate::config::NOTIFICATION_EVENT_ENCODE_FAILED),
            _ => None,
        },
        NotifiableEvent::ScanCompleted => Some(crate::config::NOTIFICATION_EVENT_SCAN_COMPLETED),
        NotifiableEvent::EngineIdle => Some(crate::config::NOTIFICATION_EVENT_ENGINE_IDLE),
    }
}

fn minutes_since_midnight(time_value: &str) -> Option<u32> {
    let mut parts = time_value.trim().split(':');
    let hour = parts.next()?.parse::<u32>().ok()?;
    let minute = parts.next()?.parse::<u32>().ok()?;
    if parts.next().is_some() || hour > 23 || minute > 59 {
        return None;
    }
    Some(hour * 60 + minute)
}

fn quiet_hours_active(
    notifications: &crate::config::NotificationsConfig,
    now: chrono::DateTime<chrono::Local>,
) -> bool {
    if !notifications.quiet_hours_enabled {
        return false;
    }

    let Some(start) = minutes_since_midnight(&notifications.quiet_hours_start_local) else {
        return false;
    };
    let Some(end) = minutes_since_midnight(&notifications.quiet_hours_end_local) else {
        return false;
    };
    if start == end {
        return false;
    }

    let current = now.hour() * 60 + now.minute();
    if start < end {
        current >= start && current < end
    } else {
        current >= start || current < end
    }
}

fn quiet_hours_suppress_event(event_key: &str) -> bool {
    event_key != crate::config::NOTIFICATION_EVENT_ENCODE_FAILED
}

impl NotificationManager {
    pub fn new(db: Db, config: Arc<RwLock<Config>>) -> Self {
        Self {
            db,
            config,
            daily_summary_last_sent: Arc::new(Mutex::new(None)),
        }
    }

    /// Build an HTTP client with SSRF protections: DNS resolution timeout,
    /// private-IP blocking (unless allow_local_notifications), no redirects,
    /// and a 10-second request timeout.
    async fn build_safe_client(&self, target: &NotificationTarget) -> NotificationResult<Client> {
        if let Some(endpoint_url) = endpoint_url_for_target(target)? {
            let url = Url::parse(&endpoint_url)?;
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
                ips.into_iter()
                    .map(|a| a.ip())
                    .next()
                    .ok_or("no IP address found for notification endpoint")?
            } else {
                ips.into_iter()
                    .map(|a| a.ip())
                    .find(|ip| !is_private_ip(*ip))
                    .ok_or("no public IP address found for notification endpoint")?
            };

            Ok(Client::builder()
                .timeout(Duration::from_secs(10))
                .redirect(Policy::none())
                .resolve(host, std::net::SocketAddr::new(target_ip, port))
                .build()?)
        } else {
            Ok(Client::builder()
                .timeout(Duration::from_secs(10))
                .redirect(Policy::none())
                .build()?)
        }
    }

    pub fn start_listener(&self, event_channels: &EventChannels) {
        let manager_clone = self.clone();
        let summary_manager = self.clone();

        // Listen for job events (state changes are the only ones we notify on)
        let mut jobs_rx = event_channels.jobs.subscribe();
        let job_manager = self.clone();
        tokio::spawn(async move {
            loop {
                match jobs_rx.recv().await {
                    Ok(JobEvent::StateChanged { job_id, status }) => {
                        let event = NotifiableEvent::JobStateChanged { job_id, status };
                        if let Err(e) = job_manager.handle_event(event).await {
                            error!("Notification error: {}", e);
                        }
                    }
                    Ok(_) => {} // Ignore Progress, Decision, Log
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        warn!("Notification job listener lagged")
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        // Listen for system events (scan completed, engine idle)
        let mut system_rx = event_channels.system.subscribe();
        tokio::spawn(async move {
            loop {
                match system_rx.recv().await {
                    Ok(SystemEvent::ScanCompleted) => {
                        if let Err(e) = manager_clone
                            .handle_event(NotifiableEvent::ScanCompleted)
                            .await
                        {
                            error!("Notification error: {}", e);
                        }
                    }
                    Ok(SystemEvent::EngineIdle) => {
                        if let Err(e) = manager_clone
                            .handle_event(NotifiableEvent::EngineIdle)
                            .await
                        {
                            error!("Notification error: {}", e);
                        }
                    }
                    Ok(_) => {} // Ignore ScanStarted, EngineStatusChanged, HardwareStateChanged
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        warn!("Notification system listener lagged")
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        tokio::spawn(async move {
            let start = tokio::time::Instant::now()
                + delay_until_next_minute_boundary(chrono::Local::now());
            let mut interval = tokio::time::interval_at(start, Duration::from_secs(60));
            loop {
                interval.tick().await;
                if let Err(err) = summary_manager
                    .maybe_send_daily_summary_at(chrono::Local::now())
                    .await
                {
                    error!("Daily summary notification error: {}", err);
                }
            }
        });
    }

    pub async fn send_test(&self, target: &NotificationTarget) -> NotificationResult<()> {
        let event = NotifiableEvent::JobStateChanged {
            job_id: 0,
            status: crate::db::JobState::Completed,
        };
        self.send(target, &event).await
    }

    async fn handle_event(&self, event: NotifiableEvent) -> NotificationResult<()> {
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

        let event_key = match event_key(&event) {
            Some(event_key) => event_key,
            None => return Ok(()),
        };

        let suppress_for_quiet_hours = {
            let config = self.config.read().await.clone();
            quiet_hours_active(&config.notifications, chrono::Local::now())
                && quiet_hours_suppress_event(event_key)
        };
        if suppress_for_quiet_hours {
            return Ok(());
        }

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

            let normalized_allowed = crate::config::normalize_notification_events(&allowed);
            if normalized_allowed
                .iter()
                .any(|candidate| candidate == event_key)
            {
                let manager = self.clone();
                let event_clone = event.clone();
                tokio::spawn(async move {
                    if let Err(e) = manager.send(&target, &event_clone).await {
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

    async fn maybe_send_daily_summary_at(
        &self,
        now: chrono::DateTime<chrono::Local>,
    ) -> NotificationResult<()> {
        let config = self.config.read().await.clone();
        let parts = config
            .notifications
            .daily_summary_time_local
            .split(':')
            .collect::<Vec<_>>();
        if parts.len() != 2 {
            return Ok(());
        }
        let hour = parts[0].parse::<u32>().unwrap_or(9);
        let minute = parts[1].parse::<u32>().unwrap_or(0);
        let Some(scheduled_at) = now
            .with_hour(hour)
            .and_then(|value| value.with_minute(minute))
            .and_then(|value| value.with_second(0))
            .and_then(|value| value.with_nanosecond(0))
        else {
            return Ok(());
        };
        if now < scheduled_at {
            return Ok(());
        }

        let summary_key = now.format("%Y-%m-%d").to_string();
        if self.daily_summary_already_sent(&summary_key).await? {
            return Ok(());
        }

        let targets = self.db.get_notification_targets().await?;
        let mut eligible_targets = Vec::new();
        for target in targets {
            if !target.enabled {
                continue;
            }
            let allowed: Vec<String> = match serde_json::from_str(&target.events) {
                Ok(events) => events,
                Err(err) => {
                    warn!(
                        "Failed to parse events for notification target '{}': {}",
                        target.name, err
                    );
                    Vec::new()
                }
            };
            let normalized_allowed = crate::config::normalize_notification_events(&allowed);
            if normalized_allowed
                .iter()
                .any(|event| event == crate::config::NOTIFICATION_EVENT_DAILY_SUMMARY)
            {
                eligible_targets.push(target);
            }
        }

        if eligible_targets.is_empty() {
            self.mark_daily_summary_sent(&summary_key).await?;
            return Ok(());
        }

        let summary = self.db.get_daily_summary_stats().await?;
        let mut delivered = 0usize;
        for target in eligible_targets {
            if let Err(err) = self.send_daily_summary_target(&target, &summary).await {
                error!(
                    "Failed to send daily summary to target '{}': {}",
                    target.name, err
                );
                continue;
            }
            delivered += 1;
        }

        if delivered > 0 {
            self.mark_daily_summary_sent(&summary_key).await?;
        }

        Ok(())
    }

    async fn daily_summary_already_sent(&self, summary_key: &str) -> NotificationResult<bool> {
        {
            let last_sent = self.daily_summary_last_sent.lock().await;
            if last_sent.as_deref() == Some(summary_key) {
                return Ok(true);
            }
        }

        let persisted = self
            .db
            .get_preference(DAILY_SUMMARY_LAST_SUCCESS_KEY)
            .await?;
        if persisted.as_deref() == Some(summary_key) {
            let mut last_sent = self.daily_summary_last_sent.lock().await;
            *last_sent = Some(summary_key.to_string());
            return Ok(true);
        }

        Ok(false)
    }

    async fn mark_daily_summary_sent(&self, summary_key: &str) -> NotificationResult<()> {
        self.db
            .set_preference(DAILY_SUMMARY_LAST_SUCCESS_KEY, summary_key)
            .await?;
        let mut last_sent = self.daily_summary_last_sent.lock().await;
        *last_sent = Some(summary_key.to_string());
        Ok(())
    }

    async fn send(
        &self,
        target: &NotificationTarget,
        event: &NotifiableEvent,
    ) -> NotificationResult<()> {
        let event_key = event_key(event).unwrap_or("unknown");
        let client = self.build_safe_client(target).await?;

        let (decision_explanation, failure_explanation) = match event {
            NotifiableEvent::JobStateChanged { job_id, status } => {
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
            "discord_webhook" => {
                self.send_discord_with_client(
                    &client,
                    target,
                    event,
                    event_key,
                    decision_explanation.as_ref(),
                    failure_explanation.as_ref(),
                )
                .await
            }
            "discord_bot" => {
                self.send_discord_bot_with_client(
                    &client,
                    target,
                    event,
                    event_key,
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
                    event_key,
                    decision_explanation.as_ref(),
                    failure_explanation.as_ref(),
                )
                .await
            }
            "ntfy" => {
                self.send_ntfy_with_client(
                    &client,
                    target,
                    event,
                    event_key,
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
                    event_key,
                    decision_explanation.as_ref(),
                    failure_explanation.as_ref(),
                )
                .await
            }
            "telegram" => {
                self.send_telegram_with_client(
                    &client,
                    target,
                    event,
                    event_key,
                    decision_explanation.as_ref(),
                    failure_explanation.as_ref(),
                )
                .await
            }
            "email" => {
                self.send_email(
                    target,
                    event,
                    event_key,
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

    fn message_for_event(
        &self,
        event: &NotifiableEvent,
        decision_explanation: Option<&Explanation>,
        failure_explanation: Option<&Explanation>,
    ) -> String {
        match event {
            NotifiableEvent::JobStateChanged { job_id, status } => self.notification_message(
                *job_id,
                status.as_ref(),
                decision_explanation,
                failure_explanation,
            ),
            NotifiableEvent::ScanCompleted => {
                "Library scan completed. Review the queue for newly discovered work.".to_string()
            }
            NotifiableEvent::EngineIdle => {
                "The engine is idle. There are no active jobs and no queued work ready to run."
                    .to_string()
            }
        }
    }

    fn daily_summary_message(&self, summary: &crate::db::DailySummaryStats) -> String {
        let mut lines = vec![
            "Daily summary".to_string(),
            format!("Completed: {}", summary.completed),
            format!("Failed: {}", summary.failed),
            format!("Skipped: {}", summary.skipped),
            format!("Bytes saved: {}", summary.bytes_saved),
        ];
        if !summary.top_failure_reasons.is_empty() {
            lines.push(format!(
                "Top failure reasons: {}",
                summary.top_failure_reasons.join(", ")
            ));
        }
        if !summary.top_skip_reasons.is_empty() {
            lines.push(format!(
                "Top skip reasons: {}",
                summary.top_skip_reasons.join(", ")
            ));
        }
        lines.join("\n")
    }

    async fn send_discord_with_client(
        &self,
        client: &Client,
        target: &NotificationTarget,
        event: &NotifiableEvent,
        event_key: &str,
        decision_explanation: Option<&Explanation>,
        failure_explanation: Option<&Explanation>,
    ) -> NotificationResult<()> {
        let config = parse_target_config::<DiscordWebhookConfig>(target)?;
        let color = match event_key {
            "encode.completed" => 0x00FF00,
            "encode.failed" => 0xFF0000,
            "encode.queued" => 0xF1C40F,
            "encode.started" => 0x3498DB,
            "daily.summary" => 0x9B59B6,
            _ => 0x95A5A6,
        };

        let message = self.message_for_event(event, decision_explanation, failure_explanation);

        let body = json!({
            "embeds": [{
                "title": "Alchemist Notification",
                "description": message,
                "color": color,
                "timestamp": chrono::Utc::now().to_rfc3339()
            }]
        });

        client
            .post(&config.webhook_url)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn send_discord_bot_with_client(
        &self,
        client: &Client,
        target: &NotificationTarget,
        event: &NotifiableEvent,
        _event_key: &str,
        decision_explanation: Option<&Explanation>,
        failure_explanation: Option<&Explanation>,
    ) -> NotificationResult<()> {
        let config = parse_target_config::<DiscordBotConfig>(target)?;
        let message = self.message_for_event(event, decision_explanation, failure_explanation);

        client
            .post(format!(
                "https://discord.com/api/v10/channels/{}/messages",
                config.channel_id
            ))
            .header("Authorization", format!("Bot {}", config.bot_token))
            .json(&json!({ "content": message }))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn send_gotify_with_client(
        &self,
        client: &Client,
        target: &NotificationTarget,
        event: &NotifiableEvent,
        event_key: &str,
        decision_explanation: Option<&Explanation>,
        failure_explanation: Option<&Explanation>,
    ) -> NotificationResult<()> {
        let config = parse_target_config::<GotifyConfig>(target)?;
        let message = self.message_for_event(event, decision_explanation, failure_explanation);

        let priority = match event_key {
            "encode.failed" => 8,
            "encode.completed" => 5,
            _ => 2,
        };

        let req = client
            .post(format!(
                "{}/message",
                config.server_url.trim_end_matches('/')
            ))
            .json(&json!({
                "title": "Alchemist",
                "message": message,
                "priority": priority,
                "extras": {
                    "client::display": {
                        "contentType": "text/plain"
                    }
                }
            }));
        req.header("X-Gotify-Key", config.app_token)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn send_ntfy_with_client(
        &self,
        client: &Client,
        target: &NotificationTarget,
        event: &NotifiableEvent,
        event_key: &str,
        decision_explanation: Option<&Explanation>,
        failure_explanation: Option<&Explanation>,
    ) -> NotificationResult<()> {
        let config = parse_target_config::<NtfyConfig>(target)?;
        let message = self.message_for_event(event, decision_explanation, failure_explanation);

        let priority = match event_key {
            "encode.failed" => "5",
            "daily.summary" => "4",
            "encode.completed" | "scan.completed" | "engine.idle" => "3",
            _ => "3",
        };

        let url = format!(
            "{}/{}",
            config.server_url.trim_end_matches('/'),
            config.topic.trim_matches('/')
        );
        let mut req = client
            .post(url)
            .header("Content-Type", "text/plain; charset=utf-8")
            .header("Title", "Alchemist")
            .header("Priority", priority)
            .body(message);
        if let Some(token) = &config.access_token {
            req = req.bearer_auth(token);
        }
        req.send().await?.error_for_status()?;
        Ok(())
    }

    async fn send_webhook_with_client(
        &self,
        client: &Client,
        target: &NotificationTarget,
        event: &NotifiableEvent,
        event_key: &str,
        decision_explanation: Option<&Explanation>,
        failure_explanation: Option<&Explanation>,
    ) -> NotificationResult<()> {
        let config = parse_target_config::<WebhookConfig>(target)?;
        let message = self.message_for_event(event, decision_explanation, failure_explanation);

        let body = json!({
            "event": event_key,
            "message": message,
            "data": event,
            "decision_explanation": decision_explanation,
            "failure_explanation": failure_explanation,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });

        let mut req = client.post(&config.url).json(&body);
        if let Some(token) = &config.auth_token {
            req = req.bearer_auth(token);
        }

        req.send().await?.error_for_status()?;
        Ok(())
    }

    async fn send_telegram_with_client(
        &self,
        client: &Client,
        target: &NotificationTarget,
        event: &NotifiableEvent,
        _event_key: &str,
        decision_explanation: Option<&Explanation>,
        failure_explanation: Option<&Explanation>,
    ) -> NotificationResult<()> {
        let config = parse_target_config::<TelegramConfig>(target)?;
        let message = self.message_for_event(event, decision_explanation, failure_explanation);

        client
            .post(format!(
                "https://api.telegram.org/bot{}/sendMessage",
                config.bot_token
            ))
            .json(&json!({
                "chat_id": config.chat_id,
                "text": message
            }))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn send_email(
        &self,
        target: &NotificationTarget,
        event: &NotifiableEvent,
        _event_key: &str,
        decision_explanation: Option<&Explanation>,
        failure_explanation: Option<&Explanation>,
    ) -> NotificationResult<()> {
        let config = parse_target_config::<EmailConfig>(target)?;
        let message_text = self.message_for_event(event, decision_explanation, failure_explanation);

        let from: Mailbox = config.from_address.parse()?;
        let mut builder = Message::builder()
            .from(from)
            .subject("Alchemist Notification");
        for address in &config.to_addresses {
            builder = builder.to(address.parse::<Mailbox>()?);
        }

        let email = builder.singlepart(
            SinglePart::builder()
                .header(ContentType::TEXT_PLAIN)
                .body(message_text),
        )?;

        let security = config
            .security
            .as_deref()
            .unwrap_or("starttls")
            .to_ascii_lowercase();

        let mut transport = match security.as_str() {
            "tls" | "smtps" => AsyncSmtpTransport::<Tokio1Executor>::relay(&config.smtp_host)?,
            "none" => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.smtp_host),
            _ => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.smtp_host)?,
        }
        .port(config.smtp_port);

        if let (Some(username), Some(password)) = (config.username.clone(), config.password.clone())
        {
            transport = transport.credentials(Credentials::new(username, password));
        }

        transport.build().send(email).await?;
        Ok(())
    }

    async fn send_daily_summary_target(
        &self,
        target: &NotificationTarget,
        summary: &crate::db::DailySummaryStats,
    ) -> NotificationResult<()> {
        let message = self.daily_summary_message(summary);
        let client = self.build_safe_client(target).await?;
        match target.target_type.as_str() {
            "discord_webhook" => {
                let config = parse_target_config::<DiscordWebhookConfig>(target)?;
                client
                    .post(config.webhook_url)
                    .json(&json!({
                        "embeds": [{
                            "title": "Alchemist Daily Summary",
                            "description": message,
                            "color": 0x9B59B6,
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        }]
                    }))
                    .send()
                    .await?
                    .error_for_status()?;
            }
            "discord_bot" => {
                let config = parse_target_config::<DiscordBotConfig>(target)?;
                client
                    .post(format!(
                        "https://discord.com/api/v10/channels/{}/messages",
                        config.channel_id
                    ))
                    .header("Authorization", format!("Bot {}", config.bot_token))
                    .json(&json!({ "content": message }))
                    .send()
                    .await?
                    .error_for_status()?;
            }
            "gotify" => {
                let config = parse_target_config::<GotifyConfig>(target)?;
                client
                    .post(config.server_url)
                    .header("X-Gotify-Key", config.app_token)
                    .json(&json!({
                        "title": "Alchemist Daily Summary",
                        "message": message,
                        "priority": 4
                    }))
                    .send()
                    .await?
                    .error_for_status()?;
            }
            "ntfy" => {
                let config = parse_target_config::<NtfyConfig>(target)?;
                let url = format!(
                    "{}/{}",
                    config.server_url.trim_end_matches('/'),
                    config.topic.trim_matches('/')
                );
                let mut req = client
                    .post(url)
                    .header("Content-Type", "text/plain; charset=utf-8")
                    .header("Title", "Alchemist Daily Summary")
                    .header("Priority", "4")
                    .body(message);
                if let Some(token) = config.access_token {
                    req = req.bearer_auth(token);
                }
                req.send().await?.error_for_status()?;
            }
            "webhook" => {
                let config = parse_target_config::<WebhookConfig>(target)?;
                let mut req = client.post(config.url).json(&json!({
                    "event": crate::config::NOTIFICATION_EVENT_DAILY_SUMMARY,
                    "summary": summary,
                    "message": message,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }));
                if let Some(token) = config.auth_token {
                    req = req.bearer_auth(token);
                }
                req.send().await?.error_for_status()?;
            }
            "telegram" => {
                let config = parse_target_config::<TelegramConfig>(target)?;
                client
                    .post(format!(
                        "https://api.telegram.org/bot{}/sendMessage",
                        config.bot_token
                    ))
                    .json(&json!({
                        "chat_id": config.chat_id,
                        "text": message
                    }))
                    .send()
                    .await?
                    .error_for_status()?;
            }
            "email" => {
                let config = parse_target_config::<EmailConfig>(target)?;
                let from: Mailbox = config.from_address.parse()?;
                let mut builder = Message::builder()
                    .from(from)
                    .subject("Alchemist Daily Summary");
                for address in &config.to_addresses {
                    builder = builder.to(address.parse::<Mailbox>()?);
                }
                let email = builder.singlepart(
                    SinglePart::builder()
                        .header(ContentType::TEXT_PLAIN)
                        .body(message),
                )?;
                let security = config
                    .security
                    .as_deref()
                    .unwrap_or("starttls")
                    .to_ascii_lowercase();
                let mut transport = match security.as_str() {
                    "tls" | "smtps" => {
                        AsyncSmtpTransport::<Tokio1Executor>::relay(&config.smtp_host)?
                    }
                    "none" => {
                        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.smtp_host)
                    }
                    _ => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.smtp_host)?,
                }
                .port(config.smtp_port);
                if let (Some(username), Some(password)) =
                    (config.username.clone(), config.password.clone())
                {
                    transport = transport.credentials(Credentials::new(username, password));
                }
                transport.build().send(email).await?;
            }
            _ => {}
        }
        Ok(())
    }
}

fn delay_until_next_minute_boundary(now: chrono::DateTime<chrono::Local>) -> Duration {
    let remaining_seconds = 60_u64.saturating_sub(now.second() as u64).max(1);
    let mut delay = Duration::from_secs(remaining_seconds);
    if now.nanosecond() > 0 {
        delay = delay
            .checked_sub(Duration::from_nanos(now.nanosecond() as u64))
            .unwrap_or_else(|| Duration::from_millis(1));
    }
    delay
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
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    fn scheduled_test_time(hour: u32, minute: u32) -> chrono::DateTime<chrono::Local> {
        chrono::Local::now()
            .with_hour(hour)
            .and_then(|value| value.with_minute(minute))
            .and_then(|value| value.with_second(0))
            .and_then(|value| value.with_nanosecond(0))
            .unwrap_or_else(chrono::Local::now)
    }

    async fn add_daily_summary_webhook_target(
        db: &Db,
        addr: std::net::SocketAddr,
    ) -> NotificationResult<()> {
        let config_json = serde_json::json!({ "url": format!("http://{}", addr) }).to_string();
        db.add_notification_target(
            "daily-summary",
            "webhook",
            &config_json,
            "[\"daily.summary\"]",
            true,
        )
        .await?;
        Ok(())
    }

    #[test]
    fn quiet_hours_detect_overnight_window() {
        let mut config = crate::config::Config::default();
        config.notifications.quiet_hours_enabled = true;
        config.notifications.quiet_hours_start_local = "22:00".to_string();
        config.notifications.quiet_hours_end_local = "08:00".to_string();

        assert!(quiet_hours_active(
            &config.notifications,
            scheduled_test_time(23, 30)
        ));
        assert!(quiet_hours_active(
            &config.notifications,
            scheduled_test_time(7, 45)
        ));
        assert!(!quiet_hours_active(
            &config.notifications,
            scheduled_test_time(12, 0)
        ));
    }

    #[test]
    fn quiet_hours_do_not_suppress_failures() {
        assert!(!quiet_hours_suppress_event(
            crate::config::NOTIFICATION_EVENT_ENCODE_FAILED
        ));
        assert!(quiet_hours_suppress_event(
            crate::config::NOTIFICATION_EVENT_ENCODE_COMPLETED
        ));
    }

    #[tokio::test]
    async fn test_webhook_errors_on_non_success()
    -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
            config_json: serde_json::json!({ "url": format!("http://{}", addr) }).to_string(),
            events: "[]".to_string(),
            enabled: true,
            created_at: chrono::Utc::now(),
        };
        let event = NotifiableEvent::JobStateChanged {
            job_id: 1,
            status: crate::db::JobState::Failed,
        };

        let result = manager.send(&target, &event).await;
        assert!(result.is_err());

        drop(manager);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }

    #[tokio::test]
    async fn webhook_payload_includes_structured_explanations()
    -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
            let (mut socket, _) = match listener.accept().await {
                Ok(socket) => socket,
                Err(err) => return Err::<String, std::io::Error>(err),
            };
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let read = socket.read(&mut chunk).await?;
                if read == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..read]);
                if buf.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            let response = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n";
            socket.write_all(response.as_bytes()).await?;
            Ok(String::from_utf8_lossy(&buf).to_string())
        });

        let target = NotificationTarget {
            id: 0,
            name: "test".to_string(),
            target_type: "webhook".to_string(),
            config_json: serde_json::json!({ "url": format!("http://{}", addr) }).to_string(),
            events: "[\"failed\"]".to_string(),
            enabled: true,
            created_at: chrono::Utc::now(),
        };
        let event = NotifiableEvent::JobStateChanged {
            job_id: job.id,
            status: JobState::Failed,
        };

        manager.send(&target, &event).await?;
        let request = body_task.await??;
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

    #[tokio::test]
    async fn daily_summary_retries_after_failed_delivery_and_marks_success()
    -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut db_path = std::env::temp_dir();
        let token: u64 = rand::random();
        db_path.push(format!("alchemist_notifications_daily_retry_{}.db", token));

        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;
        let mut test_config = crate::config::Config::default();
        test_config.notifications.allow_local_notifications = true;
        test_config.notifications.daily_summary_time_local = "09:00".to_string();
        let config = Arc::new(RwLock::new(test_config));
        let manager = NotificationManager::new(db.clone(), config);

        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                return Ok(());
            }
            Err(err) => return Err(err.into()),
        };
        let addr = listener.local_addr()?;
        add_daily_summary_webhook_target(&db, addr).await?;

        let request_count = Arc::new(AtomicUsize::new(0));
        let request_count_task = request_count.clone();
        let listener_task = tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let mut buf = [0u8; 1024];
                let _ = socket.read(&mut buf).await;
                let index = request_count_task.fetch_add(1, Ordering::SeqCst);
                let response = if index == 0 {
                    "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n"
                } else {
                    "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n"
                };
                let _ = socket.write_all(response.as_bytes()).await;
            }
        });

        let first_now = scheduled_test_time(9, 5);
        manager.maybe_send_daily_summary_at(first_now).await?;
        assert_eq!(request_count.load(Ordering::SeqCst), 1);
        assert_eq!(
            db.get_preference(DAILY_SUMMARY_LAST_SUCCESS_KEY).await?,
            None
        );

        manager
            .maybe_send_daily_summary_at(first_now + chrono::Duration::minutes(1))
            .await?;
        assert_eq!(request_count.load(Ordering::SeqCst), 2);
        assert_eq!(
            db.get_preference(DAILY_SUMMARY_LAST_SUCCESS_KEY).await?,
            Some(first_now.format("%Y-%m-%d").to_string())
        );

        listener_task.abort();
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }

    #[tokio::test]
    async fn daily_summary_is_restart_safe_after_successful_delivery()
    -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut db_path = std::env::temp_dir();
        let token: u64 = rand::random();
        db_path.push(format!(
            "alchemist_notifications_daily_restart_{}.db",
            token
        ));

        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;
        let mut test_config = crate::config::Config::default();
        test_config.notifications.allow_local_notifications = true;
        test_config.notifications.daily_summary_time_local = "09:00".to_string();
        let config = Arc::new(RwLock::new(test_config));

        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                return Ok(());
            }
            Err(err) => return Err(err.into()),
        };
        let addr = listener.local_addr()?;
        add_daily_summary_webhook_target(&db, addr).await?;

        let request_count = Arc::new(AtomicUsize::new(0));
        let request_count_task = request_count.clone();
        let listener_task = tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let mut buf = [0u8; 1024];
                let _ = socket.read(&mut buf).await;
                request_count_task.fetch_add(1, Ordering::SeqCst);
                let _ = socket
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                    .await;
            }
        });

        let first_now = scheduled_test_time(9, 2);
        let manager = NotificationManager::new(db.clone(), config.clone());
        manager.maybe_send_daily_summary_at(first_now).await?;
        assert_eq!(request_count.load(Ordering::SeqCst), 1);

        let restarted_manager = NotificationManager::new(db.clone(), config.clone());
        restarted_manager
            .maybe_send_daily_summary_at(first_now + chrono::Duration::minutes(10))
            .await?;
        assert_eq!(request_count.load(Ordering::SeqCst), 1);

        listener_task.abort();
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }

    #[tokio::test]
    async fn daily_summary_marks_day_sent_when_no_targets_are_eligible()
    -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut db_path = std::env::temp_dir();
        let token: u64 = rand::random();
        db_path.push(format!(
            "alchemist_notifications_daily_no_targets_{}.db",
            token
        ));

        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;
        let mut test_config = crate::config::Config::default();
        test_config.notifications.daily_summary_time_local = "09:00".to_string();
        let config = Arc::new(RwLock::new(test_config));
        let manager = NotificationManager::new(db.clone(), config);

        let now = scheduled_test_time(9, 1);
        manager.maybe_send_daily_summary_at(now).await?;
        assert_eq!(
            db.get_preference(DAILY_SUMMARY_LAST_SUCCESS_KEY).await?,
            Some(now.format("%Y-%m-%d").to_string())
        );

        let _ = std::fs::remove_file(db_path);
        Ok(())
    }
}
