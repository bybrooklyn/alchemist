#[cfg(feature = "hydrate")]
use crate::db::AlchemistEvent;
use crate::db::{Job, JobState};
use leptos::*;
use leptos_meta::*;
use leptos_router::*;

#[server(GetJobs, "/api")]
pub async fn get_jobs() -> Result<Vec<Job>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::db::Db;
        use axum::Extension;
        use std::sync::Arc;

        let db = use_context::<Extension<Arc<Db>>>()
            .ok_or_else(|| ServerFnError::new("DB not found"))?
            .0
            .clone();

        db.get_all_jobs()
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        unreachable!()
    }
}

#[server(GetStats, "/api")]
pub async fn get_stats() -> Result<serde_json::Value, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::db::Db;
        use axum::Extension;
        use std::sync::Arc;

        let db = use_context::<Extension<Arc<Db>>>()
            .ok_or_else(|| ServerFnError::new("DB not found"))?
            .0
            .clone();

        db.get_stats()
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        unreachable!()
    }
}

#[server(RunScan, "/api")]
pub async fn run_scan() -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::config::Config;
        use crate::Agent;
        use axum::Extension;
        use std::sync::Arc;

        let agent = use_context::<Extension<Arc<Agent>>>()
            .ok_or_else(|| ServerFnError::new("Agent not found"))?
            .0
            .clone();
        let config = use_context::<Extension<Arc<Config>>>()
            .ok_or_else(|| ServerFnError::new("Config not found"))?
            .0
            .clone();

        let dirs = config
            .scanner
            .directories
            .iter()
            .map(std::path::PathBuf::from)
            .collect();
        agent
            .scan_and_enqueue(dirs)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        unreachable!()
    }
}

#[server(CancelJob, "/api")]
pub async fn cancel_job(job_id: i64) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::Transcoder;
        use axum::Extension;
        use std::sync::Arc;

        let transcoder = use_context::<Extension<Arc<Transcoder>>>()
            .ok_or_else(|| ServerFnError::new("Transcoder not found"))?
            .0
            .clone();

        if transcoder.cancel_job(job_id) {
            Ok(())
        } else {
            Err(ServerFnError::new("Job not running or not found"))
        }
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = job_id;
        unreachable!()
    }
}

#[server(RestartJob, "/api")]
pub async fn restart_job(job_id: i64) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::db::{Db, JobState};
        use axum::Extension;
        use std::sync::Arc;

        let db = use_context::<Extension<Arc<Db>>>()
            .ok_or_else(|| ServerFnError::new("DB not found"))?
            .0
            .clone();

        db.update_job_status(job_id, JobState::Queued)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = job_id;
        unreachable!()
    }
}

#[server(RestartAllFailed, "/api")]
pub async fn restart_all_failed() -> Result<u64, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::db::{Db, JobState};
        use axum::Extension;
        use std::sync::Arc;

        let db = use_context::<Extension<Arc<Db>>>()
            .ok_or_else(|| ServerFnError::new("DB not found"))?
            .0
            .clone();

        db.batch_update_status(JobState::Failed, JobState::Queued)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        unreachable!()
    }
}

#[server(SetJobPriority, "/api")]
pub async fn set_job_priority(job_id: i64, priority: i32) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::db::Db;
        use axum::Extension;
        use std::sync::Arc;

        let db = use_context::<Extension<Arc<Db>>>()
            .ok_or_else(|| ServerFnError::new("DB not found"))?
            .0
            .clone();

        db.set_job_priority(job_id, priority)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (job_id, priority);
        unreachable!()
    }
}

#[server(GetConfig, "/api")]
pub async fn get_config() -> Result<crate::config::Config, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use crate::config::Config;
        use axum::Extension;
        use std::sync::Arc;

        let config = use_context::<Extension<Arc<Config>>>()
            .ok_or_else(|| ServerFnError::new("Config not found"))?
            .0
            .clone();

        Ok((*config).clone())
    }
    #[cfg(not(feature = "ssr"))]
    {
        unreachable!()
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/alchemist.css"/>
        <Title text="Alchemist - Transcoding Engine"/>

        <Router>
            <main class="min-h-screen bg-[#020617] text-slate-100 font-sans p-4 md:p-12">
                <Routes>
                    <Route path="" view=Dashboard/>
                </Routes>
            </main>
        </Router>
    }
}

#[component]
fn Dashboard() -> impl IntoView {
    let jobs = create_resource(
        || (),
        |_| async move { get_jobs().await.unwrap_or_default() },
    );
    let stats = create_resource(|| (), |_| async move { get_stats().await.ok() });

    // In-memory progress tracking
    let (progress_map, _set_progress_map) =
        create_signal(std::collections::HashMap::<i64, f64>::new());

    let (active_log, set_active_log) = create_signal(Option::<(i64, String)>::None);

    // SSE Effect for real-time updates
    #[cfg(feature = "hydrate")]
    create_effect(move |_| {
        use futures::StreamExt;
        use gloo_net::eventsource::futures::EventSource;

        let mut es = EventSource::new("/api/events").unwrap();
        let mut stream = es.subscribe("message").unwrap();

        spawn_local(async move {
            while let Some(Ok((_, msg))) = stream.next().await {
                if let Ok(event) =
                    serde_json::from_str::<AlchemistEvent>(&msg.data().as_string().unwrap())
                {
                    match event {
                        AlchemistEvent::JobStateChanged { .. }
                        | AlchemistEvent::Decision { .. } => {
                            jobs.refetch();
                            stats.refetch();
                        }
                        AlchemistEvent::Progress {
                            job_id, percentage, ..
                        } => {
                            _set_progress_map.update(|m| {
                                m.insert(job_id, percentage);
                            });
                        }
                        AlchemistEvent::Log { job_id, message } => {
                            set_active_log.update(|l| {
                                if let Some((id, logs)) = l {
                                    if *id == job_id {
                                        let mut new_logs = logs.clone();
                                        new_logs.push_str(&message);
                                        new_logs.push('\n');
                                        let lines: Vec<&str> =
                                            new_logs.lines().rev().take(20).collect();
                                        *l = Some((
                                            job_id,
                                            lines.into_iter().rev().collect::<Vec<_>>().join("\n"),
                                        ));
                                    }
                                }
                            });
                        }
                    }
                }
            }
        });

        on_cleanup(move || es.close());
    });

    let scan_action = create_server_action::<RunScan>();
    let cancel_action = create_server_action::<CancelJob>();
    let restart_action = create_server_action::<RestartJob>();

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
                        class="bg-blue-600 hover:bg-blue-700 text-white px-6 py-2 rounded-lg font-medium transition-all shadow-lg shadow-blue-900/20 active:scale-95"
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
                     let processing = s.as_object().map(|m| m.iter().filter(|(k, _)| ["encoding", "analyzing"].contains(&k.as_str())).map(|(_, v)| v.as_i64().unwrap_or(0)).sum::<i64>()).unwrap_or(0);
                     let failed = s.get("failed").and_then(|v| v.as_i64()).unwrap_or(0);

                     view! {
                         <StatCard label="Total Jobs" value=total.to_string() color="blue" />
                         <StatCard label="Completed" value=completed.to_string() color="emerald" />
                         <StatCard label="Active" value=processing.to_string() color="amber" />
                         <StatCard label="Failed" value=failed.to_string() color="rose" />
                     }
                 }}
            </div>

            <div class="bg-slate-900/40 backdrop-blur-md border border-slate-800/60 rounded-2xl overflow-hidden shadow-2xl">
                <div class="px-8 py-5 border-b border-slate-800/60 bg-slate-900/50 flex justify-between items-center">
                    <h2 class="text-xl font-bold text-slate-100 tracking-tight">"Engine Activity"</h2>
                    <span class="text-xs font-medium text-slate-500 bg-slate-800/50 px-3 py-1 rounded-full border border-slate-700/50">"v0.1.0"</span>
                </div>
                <div class="overflow-x-auto">
                    <table class="w-full text-left">
                        <thead class="text-xs text-slate-500 uppercase bg-slate-800/30">
                            <tr>
                                <th class="px-6 py-4">"ID"</th>
                                <th class="px-6 py-4">"File & Progress"</th>
                                <th class="px-6 py-4">"Status"</th>
                                <th class="px-6 py-4">"Actions"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800/50">
                            <Transition fallback=move || view! { <tr><td colspan="4" class="px-6 py-12 text-center text-slate-500">"Loading jobs..."</td></tr> }>
                                {move || jobs.get().map(|all_jobs| {
                                    all_jobs.into_iter().map(|job| {
                                        let id = job.id;
                                        let status_str = job.status.to_string();
                                        let status_cls = match job.status {
                                            JobState::Completed => "bg-emerald-500/10 text-emerald-400 border-emerald-500/20",
                                            JobState::Encoding | JobState::Analyzing => "bg-amber-500/10 text-amber-400 border-amber-500/20",
                                            JobState::Failed | JobState::Cancelled => "bg-rose-500/10 text-rose-400 border-rose-500/20",
                                            _ => "bg-slate-500/10 text-slate-400 border-slate-500/20",
                                        };

                                        let prog = move || progress_map.with(|m| *m.get(&id).unwrap_or(&0.0));
                                        let is_active = job.status == JobState::Encoding || job.status == JobState::Analyzing;

                                        view! {
                                            <tr class="hover:bg-slate-800/30 transition-colors group">
                                                <td class="px-6 py-4 font-mono text-xs text-slate-600">"#" {id}</td>
                                                <td class="px-6 py-4">
                                                    <div class="font-medium truncate max-w-sm text-slate-200">{job.input_path}</div>
                                                    <div class="text-xs text-slate-500 truncate mt-1">{job.decision_reason.unwrap_or_default()}</div>

                                                    {move || if is_active {
                                                        view! {
                                                            <div class="mt-3 w-full bg-slate-800 rounded-full h-1.5 overflow-hidden">
                                                                <div
                                                                    class="bg-blue-500 h-full transition-all duration-500 ease-out"
                                                                    style=format!("width: {}%", prog())
                                                                ></div>
                                                            </div>
                                                            <div class="text-[10px] text-blue-400 mt-1 font-mono uppercase tracking-wider">
                                                                {move || format!("{:.1}%", prog())}
                                                            </div>
                                                        }.into_view()
                                                    } else {
                                                        view! {}.into_view()
                                                    }}
                                                </td>
                                                <td class="px-6 py-4">
                                                    <span class=format!("px-2.5 py-1 rounded-full text-[10px] font-bold border uppercase tracking-tight {}", status_cls)>
                                                        {status_str}
                                                    </span>
                                                </td>
                                                <td class="px-6 py-4">
                                                    <div class="flex gap-2">
                                                        {move || if is_active {
                                                            view! {
                                                                <button
                                                                    on:click=move |_| cancel_action.dispatch(CancelJob { job_id: id })
                                                                    class="text-xs text-rose-400 hover:text-rose-300 font-medium px-2 py-1 rounded hover:bg-rose-500/10 transition-colors"
                                                                >
                                                                    "Cancel"
                                                                </button>
                                                                <button
                                                                    on:click=move |_| set_active_log.set(Some((id, String::new())))
                                                                    class="text-xs text-slate-400 hover:text-slate-200 font-medium px-2 py-1 rounded hover:bg-slate-500/10 transition-colors"
                                                                >
                                                                    "Logs"
                                                                </button>
                                                            }.into_view()
                                                        } else {
                                                            view! {
                                                                <button
                                                                    on:click=move |_| restart_action.dispatch(RestartJob { job_id: id })
                                                                    class="text-xs text-blue-400 hover:text-blue-300 font-medium px-2 py-1 rounded hover:bg-blue-500/10 transition-colors"
                                                                >
                                                                    "Restart"
                                                                </button>
                                                            }.into_view()
                                                        }}
                                                    </div>
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

            // Log Viewer Modal
            {move || active_log.get().map(|(id, logs)| {
                view! {
                    <div class="fixed inset-0 bg-black/80 backdrop-blur-sm z-50 flex items-center justify-center p-4">
                        <div class="bg-slate-900 border border-slate-800 rounded-2xl w-full max-w-4xl max-h-[80vh] flex flex-col shadow-2xl">
                            <div class="px-6 py-4 border-b border-slate-800 flex justify-between items-center">
                                <h3 class="font-semibold text-lg text-slate-200">"Live Logs - Job #" {id}</h3>
                                <button
                                    on:click=move |_| set_active_log.set(None)
                                    class="text-slate-500 hover:text-slate-300"
                                >
                                    <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"></path></svg>
                                </button>
                            </div>
                            <div class="p-6 overflow-y-auto flex-1 font-mono text-sm text-slate-400 bg-black/40">
                                <pre class="whitespace-pre-wrap">{logs}</pre>
                            </div>
                        </div>
                    </div>
                }
            })}
        </div>
    }
}

#[component]
fn StatCard(label: &'static str, value: String, color: &'static str) -> impl IntoView {
    let (gradient, icon_color, glow) = match color {
        "blue" => (
            "from-blue-500 to-indigo-600",
            "text-blue-400",
            "shadow-blue-500/10",
        ),
        "emerald" => (
            "from-emerald-500 to-teal-600",
            "text-emerald-400",
            "shadow-emerald-500/10",
        ),
        "amber" => (
            "from-amber-500 to-orange-600",
            "text-amber-400",
            "shadow-amber-500/10",
        ),
        "rose" => (
            "from-rose-500 to-pink-600",
            "text-rose-400",
            "shadow-rose-500/10",
        ),
        _ => (
            "from-slate-500 to-slate-600",
            "text-slate-400",
            "shadow-slate-500/10",
        ),
    };

    view! {
        <div class=format!("bg-slate-900/40 backdrop-blur-sm border border-slate-800/60 p-7 rounded-2xl transition-all duration-300 group hover:bg-slate-800/60 hover:-translate-y-1 shadow-xl {}", glow)>
            <div class=format!("{} text-[10px] font-black uppercase tracking-[0.2em] mb-4 opacity-80", icon_color)>{label}</div>
            <div class=format!("text-5xl font-black bg-gradient-to-br {} bg-clip-text text-transparent drop-shadow-sm", gradient)>
                {value}
            </div>
            <div class="mt-4 h-1 w-0 group-hover:w-full bg-gradient-to-r from-transparent via-slate-700/50 to-transparent transition-all duration-500"></div>
        </div>
    }
}
