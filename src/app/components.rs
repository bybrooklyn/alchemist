use leptos::*;

#[component]
pub fn StatCard(label: &'static str, value: String, color: &'static str) -> impl IntoView {
    let color_class = match color {
        "blue" => "text-blue-400",
        "emerald" => "text-emerald-400",
        "amber" => "text-amber-400",
        "rose" => "text-rose-400",
        _ => "text-slate-400",
    };

    view! {
        <div class="glass-card stat-card group">
            <div class=format!("stat-label {}", color_class)>{label}</div>
            <div class="stat-value">
                {value}
            </div>
            <div class="mt-4 h-1 w-0 group-hover:w-full bg-gradient-to-r from-transparent via-slate-700/50 to-transparent transition-all duration-500"></div>
        </div>
    }
}
