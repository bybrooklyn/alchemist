use crate::app::components::*;
use crate::app::server_fns::*;
#[cfg(feature = "hydrate")]
use crate::db::AlchemistEvent;
use crate::db::JobState;
use leptos::*;

#[component]
pub fn Dashboard() -> impl IntoView {
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
        <div class="dashboard-container">
            <header class="flex justify-between items-center mb-12">
                <div>
                    <h1 class="text-4xl">"Dashboard"</h1>
                    <p class="text-slate-400 mt-1">"Next-Gen Transcoding Engine"</p>
                </div>
                <div class="flex gap-4">
                    <button
                        on:click=move |_| scan_action.dispatch(RunScan {})
                        class="btn btn-primary"
                    >
                        {move || if scan_action.pending().get() { "Scanning..." } else { "Scan Now" }}
                    </button>
                </div>
            </header>

            <div class="grid-stats">
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

            <div class="glass-card" style="padding: 0; overflow: hidden;">
                <div class="px-8 py-5 border-b border-slate-800 flex justify-between items-center">
                    <h2 class="text-xl">"Engine Activity"</h2>
                    <span class="text-xs font-medium text-slate-500 bg-slate-800 px-3 py-1 rounded-full">"v0.1.0"</span>
                </div>
                <div class="overflow-x-auto">
                    <table class="w-full text-left">
                        <thead class="text-xs text-slate-500 uppercase bg-slate-800/30">
                            <tr>
                                <th class="px-6 py-4">"ID"</th>
                                <th class="px-6 py-4">"File & Progress"</th>
                                <th class="px-6 py-4">"Priority"</th>
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
                                            JobState::Encoding | JobState::Analyzing => "text-[rgb(var(--brand-primary))] border-[rgb(var(--brand-primary))/0.2] bg-[rgb(var(--brand-primary))/0.1]",
                                            JobState::Failed | JobState::Cancelled => "bg-rose-500/10 text-rose-400 border-rose-500/20",
                                            _ => "bg-slate-500/10 text-slate-400 border-slate-500/20",
                                        };

                                        let prog = move || progress_map.with(|m| *m.get(&id).unwrap_or(&0.0));
                                        let is_active = job.status == JobState::Encoding || job.status == JobState::Analyzing;

                                        view! {
                                            <tr class="transition-colors group border-b border-slate-800/30">
                                                <td class="px-6 py-4 font-mono text-xs text-slate-600">"#" {id}</td>
                                                <td class="px-6 py-4">
                                                    <div class="font-medium truncate max-w-sm text-slate-200">{job.input_path}</div>
                                                    <div class="text-xs text-slate-500 truncate mt-1">{job.decision_reason.unwrap_or_default()}</div>

                                                    {move || if is_active {
                                                        view! {
                                                            <div class="mt-3 w-full bg-slate-800 rounded-full h-1.5 overflow-hidden">
                                                                <div
                                                                    class="h-full transition-all duration-500 ease-out"
                                                                    style=format!("width: {}%; background-color: rgb(var(--brand-primary))", prog())
                                                                ></div>
                                                            </div>
                                                            <div class="text-[10px] mt-1 font-mono uppercase tracking-wider" style="color: rgb(var(--brand-primary))">
                                                                {move || format!("{:.1}%", prog())}
                                                            </div>
                                                        }.into_view()
                                                    } else {
                                                        view! {}.into_view()
                                                    }}
                                                </td>
                                                <td class="px-6 py-4">
                                                    <span class="text-xs font-mono text-slate-400">
                                                        "P" {job.priority}
                                                    </span>
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
                                                                    class="text-xs font-medium px-2 py-1 rounded transition-colors"
                                                                    style="color: rgb(var(--status-error));"
                                                                >
                                                                    "Cancel"
                                                                </button>
                                                                <button
                                                                    on:click=move |_| set_active_log.set(Some((id, String::new())))
                                                                    class="text-xs font-medium px-2 py-1 rounded transition-colors"
                                                                    style="color: rgb(var(--text-secondary));"
                                                                >
                                                                    "Logs"
                                                                </button>
                                                            }.into_view()
                                                        } else {
                                                            view! {
                                                                <button
                                                                    on:click=move |_| restart_action.dispatch(RestartJob { job_id: id })
                                                                    class="text-xs font-medium px-2 py-1 rounded transition-colors"
                                                                    style="color: rgb(var(--brand-primary));"
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
