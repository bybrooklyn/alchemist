//! Authentication, rate limiting, and security middleware.

use super::AppState;
use axum::{
    extract::{ConnectInfo, Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;
use tokio::time::Duration;

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
    headers.insert(header::X_FRAME_OPTIONS, "DENY".parse().unwrap());

    // Prevent MIME type sniffing
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());

    // XSS protection (legacy but still useful)
    headers.insert(
        "X-XSS-Protection"
            .parse::<axum::http::HeaderName>()
            .unwrap(),
        "1; mode=block".parse().unwrap(),
    );

    // Content Security Policy - allows inline scripts/styles for the SPA
    // This is permissive enough for the app while still providing protection
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; font-src 'self'; frame-ancestors 'none'"
            .parse()
            .unwrap(),
    );

    // Referrer policy
    headers.insert(
        header::REFERRER_POLICY,
        "strict-origin-when-cross-origin".parse().unwrap(),
    );

    // Permissions policy (restrict browser features)
    headers.insert(
        "Permissions-Policy"
            .parse::<axum::http::HeaderName>()
            .unwrap(),
        "geolocation=(), microphone=(), camera=()".parse().unwrap(),
    );

    response
}

pub(crate) async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path();

    // 1. API Protection: Only lock down /api routes
    if path.starts_with("/api") {
        // Public API endpoints
        if path.starts_with("/api/setup")
            || path.starts_with("/api/auth/login")
            || path.starts_with("/api/auth/logout")
            || path == "/api/health"
            || path == "/api/ready"
        {
            return next.run(req).await;
        }

        if state.setup_required.load(Ordering::Relaxed) && path == "/api/system/hardware" {
            return next.run(req).await;
        }
        if state.setup_required.load(Ordering::Relaxed) && path.starts_with("/api/fs/") {
            return next.run(req).await;
        }
        if state.setup_required.load(Ordering::Relaxed) && path == "/api/settings/bundle" {
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
            if let Ok(Some(_session)) = state.db.get_session(&t).await {
                return next.run(req).await;
            }
        }

        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }

    // 2. Static Assets / Frontend Pages
    // Allow everything else. The frontend app (Layout.astro) handles client-side redirects
    // if the user isn't authenticated, and the backend API protects the actual data.
    next.run(req).await
}

pub(crate) async fn rate_limit_middleware(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    if !req.uri().path().starts_with("/api/") {
        return next.run(req).await;
    }

    let ip = request_ip(&req).unwrap_or(IpAddr::from([0, 0, 0, 0]));
    if !allow_global_request(&state, ip).await {
        return (StatusCode::TOO_MANY_REQUESTS, "Too many requests").into_response();
    }
    next.run(req).await
}

pub(crate) async fn allow_login_attempt(state: &AppState, ip: IpAddr) -> bool {
    let mut limiter = state.login_rate_limiter.lock().await;
    let now = Instant::now();
    let cleanup_after = Duration::from_secs(60 * 60);
    limiter.retain(|_, entry| now.duration_since(entry.last_refill) <= cleanup_after);

    let entry = limiter.entry(ip).or_insert(RateLimitEntry {
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
    let entry = limiter.entry(ip).or_insert(RateLimitEntry {
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

pub(crate) fn request_ip(req: &Request) -> Option<IpAddr> {
    if let Some(xff) = req.headers().get("X-Forwarded-For") {
        if let Ok(xff_str) = xff.to_str() {
            if let Some(ip_str) = xff_str.split(',').next() {
                if let Ok(ip) = ip_str.trim().parse() {
                    return Some(ip);
                }
            }
        }
    }
    if let Some(xri) = req.headers().get("X-Real-IP") {
        if let Ok(xri_str) = xri.to_str() {
            if let Ok(ip) = xri_str.trim().parse() {
                return Some(ip);
            }
        }
    }
    req.extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip())
}
