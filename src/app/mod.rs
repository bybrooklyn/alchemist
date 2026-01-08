pub mod components;
pub mod dashboard;
pub mod server_fns;
pub mod settings;

use crate::app::dashboard::Dashboard;
use crate::app::settings::Settings;
use leptos::*;
use leptos_meta::*;
use leptos_router::*;

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/alchemist.css"/>
        <Title text="Alchemist - Transcoding Engine"/>

        <Router>
            <div class="app-container">
                <aside class="sidebar">
                    <div class="logo-container">
                        <div class="logo-icon">
                            <svg class="w-5 h-5 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19.428 15.428a2 2 0 00-1.022-.547l-2.387-.477a6 6 0 00-3.86.517l-.644.322a6 6 0 01-3.86.517l-2.387-.477a2 2 0 00-1.022.547l-1.168 1.168a2 2 0 001.61 3.412h13.44a2 2 0 001.61-3.412l-1.168-1.168zM12 5a3 3 0 100 6 3 3 0 000-6z"></path></svg>
                        </div>
                        <span class="logo-text">"Alchemist"</span>
                    </div>

                    <nav class="nav-links">
                        <SidebarLink href="/" icon="dashboard" label="Dashboard" />
                        <SidebarLink href="/settings" icon="settings" label="Settings" />
                    </nav>

                    <div class="engine-status">
                         <div class="pulse"></div>
                         <span class="text-xs font-medium text-slate-400">"Engine Online"</span>
                    </div>
                </aside>

                // Main Content
                <main class="main-content">
                    <Routes>
                        <Route path="" view=Dashboard/>
                        <Route path="/settings" view=Settings/>
                    </Routes>
                </main>
            </div>
        </Router>
    }
}

#[component]
fn SidebarLink(href: &'static str, icon: &'static str, label: &'static str) -> impl IntoView {
    let icon_svg = match icon {
        "dashboard" => view! { <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6"></path> }.into_view(),
        "settings" => view! { <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"></path><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"></path> }.into_view(),
        _ => view! { <path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"></path> }.into_view(),
    };

    view! {
        <A
            href=href
            class="nav-link"
            active_class="active"
        >
            <svg fill="none" stroke="currentColor" viewBox="0 0 24 24">
                {icon_svg}
            </svg>
            {label}
        </A>
    }
}
