#![cfg(feature = "ssr")]
use axum::{
    routing::{get, post},
    Router,
    response::sse::{Event as AxumEvent, Sse},
};
use futures::stream::Stream;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use std::convert::Infallible;
use leptos::*;
use leptos_axum::{generate_route_list, LeptosRoutes};
use std::sync::Arc;
use crate::app::*;
use crate::db::{Db, AlchemistEvent};
use crate::config::Config;
use tracing::info;
use crate::Processor;
use crate::Orchestrator;

pub async fn run_server(
    db: Arc<Db>, 
    config: Arc<Config>, 
    processor: Arc<Processor>,
    orchestrator: Arc<Orchestrator>,
    tx: broadcast::Sender<AlchemistEvent>
) -> anyhow::Result<()> {
    let conf = get_configuration(None).await.unwrap();
    let leptos_options = conf.leptos_options;
    let addr = leptos_options.site_addr;
    let routes = generate_route_list(App);

    let app = Router::new()
        .route("/api/events", get(sse_handler))
        .route("/api/*fn_name", post(leptos_axum::handle_server_fns))
        .leptos_routes(&leptos_options, routes, App)
        .fallback(leptos_axum::render_app_to_stream(leptos_options.clone(), App))
        .with_state(leptos_options)
        .layer(axum::Extension(db))
        .layer(axum::Extension(config))
        .layer(axum::Extension(processor))
        .layer(axum::Extension(orchestrator))
        .layer(axum::Extension(tx));

    info!("listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}

async fn sse_handler(
    axum::Extension(tx): axum::Extension<broadcast::Sender<AlchemistEvent>>,
) -> Sse<impl Stream<Item = Result<AxumEvent, Infallible>>> {
    let rx = tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|msg| {
        match msg {
            Ok(event) => {
                let json = serde_json::to_string(&event).ok()?;
                Some(Ok(AxumEvent::default().data(json)))
            }
            Err(_) => None,
        }
    });

    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}
