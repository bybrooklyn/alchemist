use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use crate::db::{Job, JobState};
#[cfg(feature = "hydrate")]
use crate::server::AlchemistEvent;

#[server(GetJobs, "/api")]
pub async fn get_jobs() -> Result<Vec<Job>, ServerFnError> {
    use axum::Extension;
    use std::sync::Arc;
    use crate::db::Db;

    let db = use_context::<Extension<Arc<Db>>>()
        .ok_or_else(|| ServerFnError::new("DB not found"))?
        .0.clone();

    db.get_all_jobs().await.map_err(|e| ServerFnError::new(e.to_string()))
}

#[server(GetStats, "/api")]
pub async fn get_stats() -> Result<serde_json::Value, ServerFnError> {
    use axum::Extension;
    use std::sync::Arc;
    use crate::db::Db;

    let db = use_context::<Extension<Arc<Db>>>()
        .ok_or_else(|| ServerFnError::new("DB not found"))?
        .0.clone();

    db.get_stats().await.map_err(|e| ServerFnError::new(e.to_string()))
}

#[server(RunScan, "/api")]
pub async fn run_scan() -> Result<(), ServerFnError> {
    // This is a placeholder for triggering a scan
    tracing::info!("Scan triggered via Web UI");
    Ok(())
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/alchemist.css"/>
        <Title text="Alchemist - Transcoding Engine"/>

        <Router>
            <main class="min-h-screen bg-slate-950 text-slate-100 font-sans p-4 md:p-8">
                <Routes>
                    <Route path="" view=Dashboard/>
                </Routes>
            </main>
        </Router>
    }
}

#[component]
fn Dashboard() -> impl IntoView {
    let jobs = create_resource(|| (), |_| async move { get_jobs().await.unwrap_or_default() });
    let stats = create_resource(|| (), |_| async move { get_stats().await.ok() });

    // SSE Effect for real-time updates
    #[cfg(feature = "hydrate")]
    create_effect(move |_| {
        use gloo_net::eventsource::futures::EventSource;
        use futures::StreamExt;

        let mut es = EventSource::new("/api/events").unwrap();
        let mut stream = es.subscribe("message").unwrap();

        spawn_local(async move {
            while let Some(Ok((_, msg))) = stream.next().await {
                if let Ok(event) = serde_json::from_str::<AlchemistEvent>(&msg.data().as_string().unwrap()) {
                    match event {
                        AlchemistEvent::JobStateChanged { .. } | AlchemistEvent::Decision { .. } => {
                            jobs.refetch();
                            stats.refetch();
                        }
                        _ => {}
                    }
                }
            }
        });

        on_cleanup(move || es.close());
    });

    let scan_action = create_server_action::<RunScan>();

    view! {
        <div class="max-w-6xl mx-auto">
            <header class="flex justify-between items-center mb-12">
                <div>
                    <h1 class="text-4xl font-extrabold tracking-tight bg-gradient-to-r from-blue-400 to-indigo-500 bg-clip-text text-transparent">
                        "Alchemist"
                    </h1>
                    <p class="text-slate-400 mt-1">"Next-Gen Transcoding Engine"</p>
                </div>
                <div class="flex gap-4">
                    <button 
                        on:click=move |_| scan_action.dispatch(RunScan {})
                        class="bg-blue-600 hover:bg-blue-700 text-white px-4 py-2 rounded-lg font-medium transition-colors"
                    >
                        {move || if scan_action.pending().get() { "Scanning..." } else { "Scan Now" }}
                    </button>
                </div>
            </header>

            <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6 mb-12">
                 {move || {
                     let s = stats.get().flatten().unwrap_or_else(|| serde_json::json!({}));
                     let total = s.as_object().map(|m| m.values().filter_map(|v| v.as_i64()).sum::<i64>()).unwrap_or(0);
                     let completed = s.get("completed").and_then(|v| v.as_i64()).unwrap_or(0);
                     let processing = s.get("processing").and_then(|v| v.as_i64()).unwrap_or(0);
                     let failed = s.get("failed").and_then(|v| v.as_i64()).unwrap_or(0);
                     
                     view! {
                         <StatCard label="Total Jobs" value=total.to_string() color="blue" />
                         <StatCard label="Completed" value=completed.to_string() color="emerald" />
                         <StatCard label="Processing" value=processing.to_string() color="amber" />
                         <StatCard label="Failed" value=failed.to_string() color="rose" />
                     }
                 }}
            </div>

            <div class="bg-slate-900 border border-slate-800 rounded-xl overflow-hidden">
                <div class="px-6 py-4 border-b border-slate-800 bg-slate-900/50">
                    <h2 class="text-xl font-semibold">"Recent Jobs"</h2>
                </div>
                <div class="overflow-x-auto">
                    <table class="w-full text-left">
                        <thead class="text-xs text-slate-400 uppercase bg-slate-800/50">
                            <tr>
                                <th class="px-6 py-3">"ID"</th>
                                <th class="px-6 py-3">"File"</th>
                                <th class="px-6 py-3">"Status"</th>
                                <th class="px-6 py-3">"Updated"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800">
                            <Transition fallback=move || view! { <tr><td colspan="4" class="px-6 py-8 text-center text-slate-500">"Loading jobs..."</td></tr> }>
                                {move || jobs.get().map(|all_jobs| {
                                    all_jobs.into_iter().map(|job| {
                                        let status_str = job.status.to_string();
                                        let status_cls = match job.status {
                                            JobState::Completed => "bg-emerald-500/10 text-emerald-400 border-emerald-500/20",
                                            JobState::Encoding | JobState::Analyzing => "bg-amber-500/10 text-amber-400 border-amber-500/20 animate-pulse",
                                            JobState::Failed => "bg-rose-500/10 text-rose-400 border-rose-500/20",
                                            _ => "bg-slate-500/10 text-slate-400 border-slate-500/20",
                                        };
                                        view! {
                                            <tr class="hover:bg-slate-800/50 transition-colors">
                                                <td class="px-6 py-4 font-mono text-xs text-slate-500">"#" {job.id}</td>
                                                <td class="px-6 py-4">
                                                    <div class="font-medium truncate max-w-xs">{job.input_path}</div>
                                                    <div class="text-xs text-slate-500 truncate mt-0.5">{job.decision_reason.unwrap_or_default()}</div>
                                                </td>
                                                <td class="px-6 py-4">
                                                    <span class=format!("px-2.5 py-1 rounded-full text-xs font-semibold border {}", status_cls)>
                                                        {status_str}
                                                    </span>
                                                </td>
                                                <td class="px-6 py-4 text-sm text-slate-500">
                                                    {job.updated_at.to_rfc3339()}
                                                </td>
                                            </tr>
                                        }
                                    }).collect_view()
                                })}
                            </Transition>
                        </tbody>
                    </table>
                </div>
            </div>
        </div>
    }
}

#[component]
fn StatCard(label: &'static str, value: String, color: &'static str) -> impl IntoView {
    let accent = match color {
        "blue" => "from-blue-500 to-indigo-600",
        "emerald" => "from-emerald-500 to-teal-600",
        "amber" => "from-amber-500 to-orange-600",
        "rose" => "from-rose-500 to-pink-600",
        _ => "from-slate-500 to-slate-600",
    };
    view! {
        <div class="bg-slate-900 border border-slate-800 p-6 rounded-xl hover:border-slate-700 transition-colors">
            <div class="text-slate-500 text-sm font-medium mb-2">{label}</div>
            <div class=format!("text-3xl font-bold bg-gradient-to-br {} bg-clip-text text-transparent", accent)>
                {value}
            </div>
        </div>
    }
}
