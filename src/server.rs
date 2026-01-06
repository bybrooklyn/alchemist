use axum::{
    routing::{get, post},
    Router,
    response::sse::{Event as AxumEvent, Sse},
};
use futures::stream::Stream;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use std::convert::Infallible;
use leptos::*;
use leptos_axum::{generate_route_list, LeptosRoutes};
use std::sync::Arc;
use crate::app::*;
use crate::db::{Db, JobState};
use crate::config::Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum AlchemistEvent {
    JobStateChanged { job_id: i64, status: JobState },
    Progress { job_id: i64, percentage: f64, time: String },
    Decision { job_id: i64, action: String, reason: String },
    Log { job_id: i64, message: String },
}

pub async fn run_server(db: Arc<Db>, config: Arc<Config>, tx: broadcast::Sender<AlchemistEvent>) -> anyhow::Result<()> {
    let conf = get_configuration(None).await.unwrap();
    let leptos_options = conf.leptos_options;
    let addr = leptos_options.site_addr;
    let routes = generate_route_list(App);

    let app = Router::new()
        .route("/api/events", get(sse_handler))
        .route("/api/*fn_name", post(leptos_axum::handle_server_fns))
        .leptos_routes(&leptos_options, routes, App)
        .with_state(leptos_options)
        .layer(axum::Extension(db))
        .layer(axum::Extension(config))
        .layer(axum::Extension(tx));

    // run our app with hyper
    // `axum::Server` is re-exported from `hyper`
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
