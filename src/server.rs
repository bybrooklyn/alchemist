#![cfg(feature = "ssr")]
use crate::app::*;
use crate::config::Config;
use crate::db::{AlchemistEvent, Db};
use crate::error::Result;
use crate::Agent;
use crate::Transcoder;
use axum::{
    response::sse::{Event as AxumEvent, Sse},
    routing::{get, post},
    Router,
};
use futures::stream::Stream;
use leptos::*;
use leptos_axum::{generate_route_list, LeptosRoutes};
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tower_http::services::ServeDir;
use tracing::info;

pub async fn run_server(
    db: Arc<Db>,
    config: Arc<Config>,
    agent: Arc<Agent>,
    transcoder: Arc<Transcoder>,
    tx: broadcast::Sender<AlchemistEvent>,
) -> Result<()> {
    let conf = get_configuration(Some("Cargo.toml")).await.unwrap();
    let leptos_options = conf.leptos_options;
    let addr = leptos_options.site_addr;
    let routes = generate_route_list(App);

    let pkg_path = format!(
        "{}/{}",
        leptos_options.site_root, leptos_options.site_pkg_dir
    );
    let assets_path = leptos_options.site_root.clone();

    let app = Router::new()
        .nest_service("/pkg", ServeDir::new(pkg_path))
        .nest_service("/public", ServeDir::new(assets_path))
        .route("/api/events", get(sse_handler))
        .route("/api/*fn_name", post(leptos_axum::handle_server_fns))
        .leptos_routes(&leptos_options, routes, App)
        .fallback(leptos_axum::render_app_to_stream(
            leptos_options.clone(),
            App,
        ))
        .with_state(leptos_options)
        .layer(axum::Extension(db))
        .layer(axum::Extension(config))
        .layer(axum::Extension(agent))
        .layer(axum::Extension(transcoder))
        .layer(axum::Extension(tx));

    info!("listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}

async fn sse_handler(
    axum::Extension(tx): axum::Extension<broadcast::Sender<AlchemistEvent>>,
) -> Sse<impl Stream<Item = std::result::Result<AxumEvent, Infallible>>> {
    let rx = tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|msg| match msg {
        Ok(event) => {
            let json = serde_json::to_string(&event).ok()?;
            Some(Ok(AxumEvent::default().data(json)))
        }
        Err(_) => None,
    });

    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}
