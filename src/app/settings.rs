use crate::app::server_fns::*;
use leptos::*;

#[component]
pub fn Settings() -> impl IntoView {
    let config = create_resource(|| (), |_| async move { get_config().await.ok() });

    view! {
        <div class="settings-container">
            <header class="mb-12">
                <h1 class="text-4xl">"Settings"</h1>
                <p class="text-slate-400 mt-1">"Manage engine configuration and hardware preferences"</p>
            </header>

            <div class="space-y-8">
                <Transition fallback=move || view! { <div class="text-slate-500">"Loading configuration..."</div> }>
                    {move || config.get().map(|cfg| {
                        cfg.map(|c| view! {
                            <div class="glass-card mb-8">
                                <h2 class="text-xl mb-6 flex items-center gap-2">
                                    <svg class="w-5 h-5 text-blue-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"></path><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"></path></svg>
                                    "General Configuration"
                                </h2>
                                <div class="grid grid-cols-1 md:grid-cols-2 gap-x-12 gap-y-8">
                                    <ConfigItem label="Database Path" value="alchemist.db".to_string() />
                                    <ConfigItem label="Concurrent Jobs" value=c.transcode.concurrent_jobs.to_string() />
                                    <ConfigItem label="CPU Fallback" value=c.hardware.allow_cpu_fallback.to_string() />
                                    <ConfigItem label="CPU Encoder" value=c.hardware.allow_cpu_encoding.to_string() />
                                </div>
                            </div>

                            <div class="glass-card">
                                <h2 class="text-xl mb-6 flex items-center gap-2">
                                    <svg class="w-5 h-5 text-emerald-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2m-6 9l2 2 4-4"></path></svg>
                                    "Library Folders"
                                </h2>
                                <div class="space-y-3">
                                    {c.scanner.directories.iter().map(|dir| view! {
                                        <div class="flex items-center gap-3 p-3 bg-slate-800/40 border border-slate-700/50 rounded-xl text-slate-300 font-mono text-sm">
                                            <svg class="w-4 h-4 text-slate-500" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"></path></svg>
                                            {dir}
                                        </div>
                                    }).collect_view()}
                                </div>
                            </div>
                        })
                    })}
                </Transition>
            </div>
        </div>
    }
}

#[component]
fn ConfigItem(label: &'static str, value: String) -> impl IntoView {
    view! {
        <div class="flex flex-col gap-1">
            <span class="text-xs font-bold text-slate-500 uppercase tracking-widest">{label}</span>
            <span class="text-slate-200 font-medium">{value}</span>
        </div>
    }
}
