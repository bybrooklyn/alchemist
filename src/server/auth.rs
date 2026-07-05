//! Authentication handlers: login, logout, session management.

use super::middleware::{allow_login_attempt, get_cookie_value, resolved_client_ip};
use super::{AppState, api_error_response};
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordVerifier},
};
use axum::{
    extract::{ConnectInfo, Request, State},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
};
use chrono::Utc;
use rand::RngExt;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::error;

#[derive(serde::Deserialize)]
pub(crate) struct LoginPayload {
    username: String,
    password: String,
}

pub(crate) async fn login_handler(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    axum::Json(payload): axum::Json<LoginPayload>,
) -> impl IntoResponse {
    let client_ip = resolved_client_ip(Some(addr.ip()), &headers, &state.trusted_proxies)
        .unwrap_or_else(|| addr.ip());
    if !allow_login_attempt(&state, client_ip).await {
        return api_error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "AUTH_RATE_LIMITED",
            "Too many requests",
        );
    }

    let mut is_valid = true;
    let user_result = match state.db.get_user_by_username(&payload.username).await {
        Ok(user) => user,
        Err(err) => {
            error!("Login lookup failed for '{}': {}", payload.username, err);
            // Never surface the raw database error to unauthenticated callers;
            // it can leak schema/connection details. Keep the detail generic.
            return api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "AUTH_LOOKUP_FAILED",
                "Authentication backend error",
            );
        }
    };

    // A valid argon2 static hash of a random string used to simulate work and equalize timing
    const DUMMY_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$c2FsdHN0cmluZzEyMzQ1Ng$1tJ2tA109qj15m3u5+kS/sX5X1UoZ6/H9b/30tX9N/g";

    let password = payload.password.clone();
    let hash_to_verify = match &user_result {
        Some(u) => u.password_hash.clone(),
        None => DUMMY_HASH.to_string(),
    };

    let argon2_valid = tokio::task::spawn_blocking(move || {
        let parsed_hash = match PasswordHash::new(&hash_to_verify) {
            Ok(h) => h,
            Err(_) => return false,
        };
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok()
    })
    .await
    .unwrap_or(false);

    if !argon2_valid || user_result.is_none() {
        is_valid = false;
    }

    let Some(user) = user_result.filter(|_| is_valid) else {
        return api_error_response(
            StatusCode::UNAUTHORIZED,
            "AUTH_INVALID_CREDENTIALS",
            "Invalid credentials",
        );
    };

    // Create session
    let token: String = rand::rng()
        .sample_iter(rand::distr::Alphanumeric)
        .take(64)
        .map(char::from)
        .collect();

    let expires_at = Utc::now() + chrono::Duration::days(30);

    if let Err(e) = state.db.create_session(user.id, &token, expires_at).await {
        return api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "AUTH_SESSION_CREATE_FAILED",
            format!("Failed to create session: {}", e),
        );
    }

    let cookie = build_session_cookie(&token);
    (
        [(header::SET_COOKIE, cookie)],
        axum::Json(serde_json::json!({ "status": "ok" })),
    )
        .into_response()
}

pub(crate) async fn logout_handler(
    State(state): State<Arc<AppState>>,
    req: Request,
) -> impl IntoResponse {
    let token = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|auth_str| auth_str.strip_prefix("Bearer ").map(str::to_string))
        .or_else(|| get_cookie_value(req.headers(), "alchemist_session"));

    if let Some(t) = token {
        let _ = state.db.delete_session(&t).await;
    }

    let cookie = build_clear_session_cookie();
    (
        [(header::SET_COOKIE, cookie)],
        axum::Json(serde_json::json!({ "status": "ok" })),
    )
        .into_response()
}

pub(crate) fn build_session_cookie(token: &str) -> String {
    let cookie = format!(
        "alchemist_session={}; HttpOnly; SameSite=Lax; Path=/; Max-Age=2592000",
        token
    );
    apply_secure_cookie_flag(cookie, secure_cookie_enabled())
}

pub(crate) fn build_clear_session_cookie() -> String {
    let cookie = "alchemist_session=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0".to_string();
    apply_secure_cookie_flag(cookie, secure_cookie_enabled())
}

fn apply_secure_cookie_flag(mut cookie: String, secure: bool) -> String {
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

fn secure_cookie_enabled_from_value(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

fn secure_cookie_enabled() -> bool {
    // Default to false — Alchemist serves plain HTTP.
    // Set ALCHEMIST_COOKIE_SECURE=true only when
    // running behind a TLS-terminating reverse proxy.
    secure_cookie_enabled_from_value(std::env::var("ALCHEMIST_COOKIE_SECURE").ok().as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_cookies_are_not_secure_by_default() {
        let session_cookie = apply_secure_cookie_flag(
            "alchemist_session=token; HttpOnly; SameSite=Lax; Path=/; Max-Age=2592000".to_string(),
            secure_cookie_enabled_from_value(None),
        );
        let clear_cookie = apply_secure_cookie_flag(
            "alchemist_session=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0".to_string(),
            secure_cookie_enabled_from_value(None),
        );

        assert!(!session_cookie.contains("; Secure"));
        assert!(!clear_cookie.contains("; Secure"));
    }

    #[test]
    fn session_cookies_include_secure_when_enabled() {
        let session_cookie = apply_secure_cookie_flag(
            "alchemist_session=token; HttpOnly; SameSite=Lax; Path=/; Max-Age=2592000".to_string(),
            secure_cookie_enabled_from_value(Some("true")),
        );
        let clear_cookie = apply_secure_cookie_flag(
            "alchemist_session=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0".to_string(),
            secure_cookie_enabled_from_value(Some("true")),
        );

        assert_eq!(
            session_cookie,
            apply_secure_cookie_flag(
                "alchemist_session=token; HttpOnly; SameSite=Lax; Path=/; Max-Age=2592000"
                    .to_string(),
                secure_cookie_enabled_from_value(Some("true")),
            )
        );
        assert!(clear_cookie.contains("; Secure"));
    }
}
