use leptos::*;

#[component]
pub fn StatCard(label: &'static str, value: String, color: &'static str) -> impl IntoView {
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
