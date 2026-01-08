use leptos::*;

#[component]
pub fn StatCard(label: &'static str, value: String, color: &'static str) -> impl IntoView {
    let color_style = match color {
        "blue" => "color: rgb(var(--brand-primary))",
        "emerald" => "color: rgb(var(--status-success))",
        "amber" => "color: rgb(var(--status-warning))",
        "rose" => "color: rgb(var(--status-error))",
        _ => "color: rgb(var(--text-secondary))",
    };

    view! {
        <div class="stat-card">
            <div class="stat-label" style=color_style>{label}</div>
            <div class="stat-value">
                {value}
            </div>
        </div>
    }
}
