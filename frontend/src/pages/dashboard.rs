use crate::components::sidebar::DashboardView;
use crate::components::Sidebar;
use crate::pages::{ExamplePage, Upload};
use leptos::*;

/// Página principal del Dashboard que contiene el menú lateral y las subpáginas
#[component]
pub fn Dashboard() -> impl IntoView {
    // Estado para la vista actual del dashboard
    let (current_view, set_current_view) = create_signal(DashboardView::Upload);

    view! {
        <div class="flex min-h-screen bg-gray-50">
            // Sidebar
            <Sidebar
                current_view=current_view
                on_view_change=move |view| set_current_view.set(view)
            />

            // Contenido principal con transiciones suaves
            <main class="flex-1 overflow-y-auto">
                <div class="max-w-7xl mx-auto p-8">
                    {move || {
                        let current = current_view.get();
                        view! {
                            <div
                                class="transition-all duration-300 ease-in-out animate-fade-in"
                                key=format!("{:?}", current)
                            >
                                {match current {
                                    DashboardView::Upload => view! { <Upload /> }.into_view(),
                                    DashboardView::History => view! { <TicketHistory /> }.into_view(),
                                    DashboardView::Example => view! { <ExamplePage /> }.into_view(),
                                }}
                            </div>
                        }
                    }}
                </div>
            </main>
        </div>
    }
}
