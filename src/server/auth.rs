//! Authentication handlers: login, logout, session management.

use super::AppState;
use super::middleware::{allow_login_attempt, get_cookie_value};
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordVerifier},
};
use axum::{
    extract::{ConnectInfo, Request, State},
    http::{StatusCode, header},
    response::IntoResponse,
};
use chrono::Utc;
use rand::Rng;
use std::net::SocketAddr;
use std::sync::Arc;

#[derive(serde::Deserialize)]
pub(crate) struct LoginPayload {
    username: String,
    password: String,
}

pub(crate) async fn login_handler(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    axum::Json(payload): axum::Json<LoginPayload>,
) -> impl IntoResponse {
    if !allow_login_attempt(&state, addr.ip()).await {
        return (StatusCode::TOO_MANY_REQUESTS, "Too many requests").into_response();
    }

    let mut is_valid = true;
    let user_result = state
        .db
        .get_user_by_username(&payload.username)
        .await
        .unwrap_or(None);

    // A valid argon2 static hash of a random string used to simulate work and equalize timing
    const DUMMY_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$c2FsdHN0cmluZzEyMzQ1Ng$1tJ2tA109qj15m3u5+kS/sX5X1UoZ6/H9b/30tX9N/g";

    let parsed_hash = match &user_result {
        Some(u) => PasswordHash::new(&u.password_hash).unwrap_or_else(|_| {
            is_valid = false;
            PasswordHash::new(DUMMY_HASH).expect("DUMMY_HASH must be a valid argon2 hash")
        }),
        None => {
            is_valid = false;
            PasswordHash::new(DUMMY_HASH).expect("DUMMY_HASH must be a valid argon2 hash")
        }
    };

    if Argon2::default()
        .verify_password(payload.password.as_bytes(), &parsed_hash)
        .is_err()
    {
        is_valid = false;
    }

    let Some(user) = user_result.filter(|_| is_valid) else {
        return (StatusCode::UNAUTHORIZED, "Invalid credentials").into_response();
    };

    // Create session
    let token: String = rand::rng()
        .sample_iter(rand::distr::Alphanumeric)
        .take(64)
        .map(char::from)
        .collect();

    let expires_at = Utc::now() + chrono::Duration::days(30);

    if let Err(e) = state.db.create_session(user.id, &token, expires_at).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create session: {}", e),
        )
            .into_response();
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
    let mut cookie = format!(
        "alchemist_session={}; HttpOnly; SameSite=Lax; Path=/; Max-Age=2592000",
        token
    );
    if secure_cookie_enabled() {
        cookie.push_str("; Secure");
    }
    cookie
}

pub(crate) fn build_clear_session_cookie() -> String {
    let mut cookie = "alchemist_session=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0".to_string();
    if secure_cookie_enabled() {
        cookie.push_str("; Secure");
    }
    cookie
}

fn secure_cookie_enabled() -> bool {
    // Default to false — Alchemist serves plain HTTP.
    // Set ALCHEMIST_COOKIE_SECURE=true only when
    // running behind a TLS-terminating reverse proxy.
    match std::env::var("ALCHEMIST_COOKIE_SECURE") {
        Ok(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => false,
    }
}
