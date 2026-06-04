//! Authentication, rate limiting, and security middleware.

use super::{API_REQUEST_CONTEXT, ApiRequestContext, AppState, api_error_response};
use crate::db::ApiTokenAccessLevel;
use axum::{
    extract::{ConnectInfo, Request, State},
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, header},
    middleware::Next,
    response::Response,
};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;
use tokio::time::Duration;
use uuid::Uuid;

pub(crate) struct RateLimitEntry {
    pub(crate) tokens: f64,
    pub(crate) last_refill: Instant,
}

pub(crate) const LOGIN_RATE_LIMIT_CAPACITY: f64 = 10.0;
pub(crate) const LOGIN_RATE_LIMIT_REFILL_PER_SEC: f64 = 1.0;
pub(crate) const GLOBAL_RATE_LIMIT_CAPACITY: f64 = 120.0;
pub(crate) const GLOBAL_RATE_LIMIT_REFILL_PER_SEC: f64 = 60.0;

/// Middleware to add security headers to all responses.
pub(crate) async fn security_headers_middleware(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    // Prevent clickjacking
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));

    // Prevent MIME type sniffing
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );

    // XSS protection (legacy but still useful)
    headers.insert(
        HeaderName::from_static("x-xss-protection"),
        HeaderValue::from_static("1; mode=block"),
    );

    // Content Security Policy - allows inline scripts/styles for the SPA
    // This is permissive enough for the app while still providing protection
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; font-src 'self'; frame-ancestors 'none'",
        ),
    );

    // Referrer policy
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );

    // Permissions policy (restrict browser features)
    headers.insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("geolocation=(), microphone=(), camera=()"),
    );

    response
}

pub(crate) async fn request_context_middleware(request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| valid_request_id(value))
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let context = ApiRequestContext {
        request_id: request_id.clone(),
        path,
    };

    let mut response = API_REQUEST_CONTEXT.scope(context, next.run(request)).await;
    if !response.headers().contains_key("x-request-id") {
        if let Ok(value) = HeaderValue::from_str(&request_id) {
            response.headers_mut().insert("x-request-id", value);
        }
    }
    response
}

fn valid_request_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':'))
}

pub(crate) async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path();
    let method = req.method().clone();
    let normalized_path = normalize_api_path(path);

    if state.setup_required.load(Ordering::Relaxed)
        && normalized_path != "/api/health"
        && normalized_path != "/api/ready"
    {
        let allowed = if let Some(expected_token) = &state.setup_token {
            // Token mode: require `?token=<value>` regardless of client IP.
            req.uri()
                .query()
                .and_then(|q| q.split('&').find_map(|pair| pair.strip_prefix("token=")))
                .map(|t| t == expected_token.as_str())
                .unwrap_or(false)
        } else {
            request_is_lan(&req, &state.trusted_proxies)
        };

        if !allowed {
            return api_error_response(
                StatusCode::FORBIDDEN,
                "SETUP_ACCESS_FORBIDDEN",
                "Alchemist setup is only available from the local network",
            );
        }
    }

    // Prometheus `/metrics` is intentionally unauthenticated to match standard
    // scrape conventions, but it must only respond to local-network callers.
    // Operators behind a reverse proxy can list the proxy in `trusted_proxies`
    // so the resolved client IP is used for the LAN check.
    if normalized_path == "/metrics" {
        if request_is_lan(&req, &state.trusted_proxies) {
            return next.run(req).await;
        }
        return api_error_response(
            StatusCode::FORBIDDEN,
            "METRICS_LAN_ONLY",
            "Metrics are only available from the local network",
        );
    }

    // 1. API Protection: Only lock down /api routes
    if path.starts_with("/api") {
        // Public API endpoints
        if normalized_path.starts_with("/api/setup")
            || normalized_path.starts_with("/api/auth/login")
            || normalized_path.starts_with("/api/auth/logout")
            || normalized_path == "/api/health"
            || normalized_path == "/api/ready"
        {
            return next.run(req).await;
        }

        if state.setup_required.load(Ordering::Relaxed) && normalized_path == "/api/system/hardware"
        {
            return next.run(req).await;
        }
        if state.setup_required.load(Ordering::Relaxed) && normalized_path.starts_with("/api/fs/") {
            return next.run(req).await;
        }
        if state.setup_required.load(Ordering::Relaxed) && normalized_path == "/api/settings/bundle"
        {
            return next.run(req).await;
        }

        // Protected API endpoints -> Require Token
        let mut token = req
            .headers()
            .get("Authorization")
            .and_then(|h| h.to_str().ok())
            .and_then(|auth_str| auth_str.strip_prefix("Bearer ").map(str::to_string));

        if token.is_none() {
            token = get_cookie_value(req.headers(), "alchemist_session");
        }

        if let Some(t) = token {
            match state.db.get_session(&t).await {
                Ok(Some(_session)) => return next.run(req).await,
                Ok(None) => {}
                Err(err) => {
                    tracing::error!(error = %err, "session lookup failed");
                    return api_error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "AUTH_BACKEND_UNAVAILABLE",
                        "Authentication backend unavailable",
                    );
                }
            }

            match state.db.get_active_api_token(&t).await {
                Ok(Some(api_token)) => {
                    let _ = state.db.update_api_token_last_used(api_token.id).await;
                    match api_token.access_level {
                        ApiTokenAccessLevel::FullAccess => return next.run(req).await,
                        ApiTokenAccessLevel::ReadOnly => {
                            if read_only_api_token_allows(&method, normalized_path.as_str()) {
                                return next.run(req).await;
                            }
                            return api_error_response(
                                StatusCode::FORBIDDEN,
                                "API_TOKEN_FORBIDDEN",
                                "Forbidden",
                            );
                        }
                        ApiTokenAccessLevel::ArrWebhook => {
                            if arr_webhook_api_token_allows(&method, normalized_path.as_str()) {
                                return next.run(req).await;
                            }
                            return api_error_response(
                                StatusCode::FORBIDDEN,
                                "API_TOKEN_FORBIDDEN",
                                "Forbidden",
                            );
                        }
                        ApiTokenAccessLevel::Jellyfin => {
                            if jellyfin_api_token_allows(&method, normalized_path.as_str()) {
                                return next.run(req).await;
                            }
                            return api_error_response(
                                StatusCode::FORBIDDEN,
                                "API_TOKEN_FORBIDDEN",
                                "Forbidden",
                            );
                        }
                    }
                }
                Ok(None) => {}
                Err(err) => {
                    tracing::error!(error = %err, "api token lookup failed");
                    return api_error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "AUTH_BACKEND_UNAVAILABLE",
                        "Authentication backend unavailable",
                    );
                }
            }
        }

        return api_error_response(StatusCode::UNAUTHORIZED, "AUTH_REQUIRED", "Unauthorized");
    }

    // 2. Static Assets / Frontend Pages
    // Allow everything else. The frontend app (Layout.astro) handles client-side redirects
    // if the user isn't authenticated, and the backend API protects the actual data.
    next.run(req).await
}

fn normalize_api_path(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("/api/v1") {
        if rest.is_empty() {
            return "/api".to_string();
        }
        return format!("/api{rest}");
    }
    path.to_string()
}

fn request_is_lan(req: &Request, trusted_proxies: &[IpAddr]) -> bool {
    let direct_peer = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| normalize_ip(info.0.ip()));
    let resolved = request_ip(req, trusted_proxies);

    // If resolved IP differs from direct peer, forwarded headers were used.
    // Warn operators so misconfigured proxies surface in logs.
    if let (Some(peer), Some(resolved_ip)) = (direct_peer, resolved) {
        if peer != resolved_ip && is_lan_ip(resolved_ip) {
            tracing::warn!(
                peer_ip = %peer,
                resolved_ip = %resolved_ip,
                "Setup gate: access permitted via forwarded headers. \
                 Verify your reverse proxy is forwarding client IPs correctly \
                 (X-Forwarded-For / X-Real-IP). Misconfigured proxies may \
                 expose setup to public traffic."
            );
        }
    }

    resolved.is_some_and(is_lan_ip)
}

fn read_only_api_token_allows(method: &Method, path: &str) -> bool {
    if *method != Method::GET && *method != Method::HEAD {
        return false;
    }

    if path == "/api/health"
        || path == "/api/ready"
        || path == "/api/events"
        || path == "/api/stats"
        || path == "/api/stats/aggregated"
        || path == "/api/stats/queue-eta"
        || path == "/api/stats/daily"
        || path == "/api/stats/detailed"
        || path == "/api/stats/savings"
        || path == "/api/jobs"
        || path == "/api/jobs/table"
        || path == "/api/logs/history"
        || path == "/api/engine/status"
        || path == "/api/engine/mode"
        || path == "/api/system/resources"
        || path == "/api/system/info"
        || path == "/api/system/update"
        || path == "/api/system/hardware"
        || path == "/api/system/hardware/probe-log"
        || path == "/api/library/intelligence"
        || path == "/api/library/health"
        || path == "/api/library/health/issues"
        || path.starts_with("/api/jobs/") && path.ends_with("/details")
    {
        return true;
    }

    false
}

fn arr_webhook_api_token_allows(method: &Method, path: &str) -> bool {
    *method == Method::POST && path == "/api/webhooks/arr"
}

fn jellyfin_api_token_allows(method: &Method, path: &str) -> bool {
    if *method == Method::POST && path == "/api/jobs/enqueue" {
        return true;
    }

    if *method != Method::GET && *method != Method::HEAD {
        return false;
    }

    path == "/api/system/info"
        || path == "/api/ready"
        || path == "/api/events"
        || path.starts_with("/api/jobs/") && path.ends_with("/details")
}

pub(crate) async fn rate_limit_middleware(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    if !req.uri().path().starts_with("/api/") {
        return next.run(req).await;
    }

    let ip = request_ip(&req, &state.trusted_proxies).unwrap_or(IpAddr::from([0, 0, 0, 0]));
    if !allow_global_request(&state, ip).await {
        return api_error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "RATE_LIMIT_EXCEEDED",
            "Too many requests",
        );
    }
    next.run(req).await
}

pub(crate) async fn allow_login_attempt(state: &AppState, ip: IpAddr) -> bool {
    let mut limiter = state.login_rate_limiter.lock().await;
    let now = Instant::now();
    let cleanup_after = Duration::from_secs(60 * 60);
    limiter.retain(|_, entry| now.duration_since(entry.last_refill) <= cleanup_after);

    let entry = limiter.entry(normalize_ip(ip)).or_insert(RateLimitEntry {
        tokens: LOGIN_RATE_LIMIT_CAPACITY,
        last_refill: now,
    });

    let elapsed = now.duration_since(entry.last_refill).as_secs_f64();
    if elapsed > 0.0 {
        let refill = elapsed * LOGIN_RATE_LIMIT_REFILL_PER_SEC;
        entry.tokens = (entry.tokens + refill).min(LOGIN_RATE_LIMIT_CAPACITY);
        entry.last_refill = now;
    }

    if entry.tokens >= 1.0 {
        entry.tokens -= 1.0;
        true
    } else {
        false
    }
}

async fn allow_global_request(state: &AppState, ip: IpAddr) -> bool {
    let mut limiter = state.global_rate_limiter.lock().await;
    let now = Instant::now();
    let cleanup_after = Duration::from_secs(60 * 60);
    limiter.retain(|_, entry| now.duration_since(entry.last_refill) <= cleanup_after);
    let entry = limiter.entry(normalize_ip(ip)).or_insert(RateLimitEntry {
        tokens: GLOBAL_RATE_LIMIT_CAPACITY,
        last_refill: now,
    });

    let elapsed = now.duration_since(entry.last_refill).as_secs_f64();
    if elapsed > 0.0 {
        let refill = elapsed * GLOBAL_RATE_LIMIT_REFILL_PER_SEC;
        entry.tokens = (entry.tokens + refill).min(GLOBAL_RATE_LIMIT_CAPACITY);
        entry.last_refill = now;
    }

    if entry.tokens >= 1.0 {
        entry.tokens -= 1.0;
        true
    } else {
        false
    }
}

pub(crate) fn get_cookie_value(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
    for part in cookie_header.split(';') {
        let mut iter = part.trim().splitn(2, '=');
        let key = iter.next()?.trim();
        let value = iter.next()?.trim();
        if key == name {
            return Some(value.to_string());
        }
    }
    None
}

pub(crate) fn request_ip(req: &Request, trusted_proxies: &[IpAddr]) -> Option<IpAddr> {
    let peer_ip = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip());

    resolved_client_ip(peer_ip, req.headers(), trusted_proxies)
}

pub(crate) fn resolved_client_ip(
    peer_ip: Option<IpAddr>,
    headers: &HeaderMap,
    trusted_proxies: &[IpAddr],
) -> Option<IpAddr> {
    let peer_ip = peer_ip.map(normalize_ip);

    // Only trust proxy headers (X-Forwarded-For, X-Real-IP) when the direct
    // TCP peer is a trusted reverse proxy. When trusted_proxies is non-empty,
    // only those exact IPs (plus loopback) are trusted. Otherwise, fall back
    // to trusting all RFC-1918 private ranges (legacy behaviour).
    if let Some(peer) = peer_ip {
        if is_trusted_peer(peer, trusted_proxies) {
            if let Some(xff) = headers.get("X-Forwarded-For") {
                if let Ok(xff_str) = xff.to_str() {
                    if let Some(ip_str) = xff_str.split(',').next() {
                        if let Ok(ip) = ip_str.trim().parse() {
                            return Some(normalize_ip(ip));
                        }
                    }
                }
            }
            if let Some(xri) = headers.get("X-Real-IP") {
                if let Ok(xri_str) = xri.to_str() {
                    if let Ok(ip) = xri_str.trim().parse() {
                        return Some(normalize_ip(ip));
                    }
                }
            }
        }
    }

    peer_ip
}

/// Returns true if the peer IP may be trusted to set forwarded headers.
///
/// When `trusted_proxies` is non-empty, only loopback addresses and the
/// explicitly configured IPs are trusted, tightening the default which
/// previously trusted all RFC-1918 private ranges.
fn is_trusted_peer(ip: IpAddr, trusted_proxies: &[IpAddr]) -> bool {
    let ip = normalize_ip(ip);
    let is_loopback = match ip {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => v6.is_loopback(),
    };
    if is_loopback {
        return true;
    }
    if trusted_proxies.is_empty() {
        // Legacy: trust all private ranges when no explicit list is configured.
        match ip {
            IpAddr::V4(v4) => v4.is_private() || v4.is_link_local(),
            IpAddr::V6(v6) => v6.is_unique_local() || v6.is_unicast_link_local(),
        }
    } else {
        trusted_proxies
            .iter()
            .copied()
            .map(normalize_ip)
            .any(|trusted| trusted == ip)
    }
}

fn is_lan_ip(ip: IpAddr) -> bool {
    match normalize_ip(ip) {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unique_local() || v6.is_unicast_link_local(),
    }
}

fn normalize_ip(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V4(_) => ip,
        IpAddr::V6(v6) => v6
            .to_ipv4_mapped()
            .map(IpAddr::V4)
            .unwrap_or(IpAddr::V6(v6)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn spoofed_forwarding_headers_are_ignored_for_untrusted_peers() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Forwarded-For", HeaderValue::from_static("192.168.1.25"));

        let peer = IpAddr::from([203, 0, 113, 10]);
        assert_eq!(resolved_client_ip(Some(peer), &headers, &[]), Some(peer));
    }

    #[test]
    fn trusted_proxy_forwarding_uses_and_normalizes_the_first_client_ip() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "X-Forwarded-For",
            HeaderValue::from_static("::ffff:203.0.113.20, 10.0.0.2"),
        );

        let Ok(peer) = "::ffff:10.0.0.2".parse::<IpAddr>() else {
            panic!("mapped proxy IP should parse");
        };
        let trusted_proxy = IpAddr::from([10, 0, 0, 2]);

        assert_eq!(
            resolved_client_ip(Some(peer), &headers, &[trusted_proxy]),
            Some(IpAddr::from([203, 0, 113, 20]))
        );
    }

    #[test]
    fn mapped_ipv6_addresses_are_normalized_for_lan_and_proxy_checks() {
        let Ok(mapped_loopback) = "::ffff:127.0.0.1".parse::<IpAddr>() else {
            panic!("mapped loopback should parse");
        };
        let Ok(mapped_lan) = "::ffff:192.168.1.25".parse::<IpAddr>() else {
            panic!("mapped LAN address should parse");
        };
        let Ok(mapped_public) = "::ffff:203.0.113.10".parse::<IpAddr>() else {
            panic!("mapped public address should parse");
        };

        assert!(is_lan_ip(mapped_loopback));
        assert!(is_lan_ip(mapped_lan));
        assert!(!is_lan_ip(mapped_public));
        assert!(is_trusted_peer(
            mapped_public,
            &[IpAddr::from([203, 0, 113, 10])]
        ));
    }
}
