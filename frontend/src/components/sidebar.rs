use leptos::*;

#[derive(Debug, Clone, PartialEq)]
pub enum DashboardView {
    Upload,
    History,
    Stats,
    MonthlyEvolution,
    Prediction,
}

#[component]
pub fn Sidebar(
    #[prop(into)] current_view: Signal<DashboardView>,
    #[prop(into)] on_view_change: Callback<DashboardView>,
) -> impl IntoView {
    // Obtener email del usuario desde localStorage
    // Obtener estado demo
    let is_demo_user = create_memo(move |_| {
        if let Some(window) = web_sys::window() {
            if let Ok(Some(storage)) = window.local_storage() {
                if let Ok(Some(demo)) = storage.get_item("user_is_demo") {
                    return demo == "true";
                }
            }
        }
        false
    });

    let user_email = create_memo(move |_| {
        if let Some(window) = web_sys::window() {
            if let Ok(Some(storage)) = window.local_storage() {
                if let Ok(Some(email)) = storage.get_item("user_email") {
                    return email;
                }
            }
        }
        "Usuario".to_string()
    });

    view! {
        <aside class="w-64 bg-white border-r border-gray-200 flex flex-col h-screen sticky top-0">
            // Header con logo
            <div class="p-6 border-b border-gray-200">
                <div class="flex items-center gap-3">
                    <div class="flex items-center justify-center w-10 h-10 bg-primary-600 rounded-xl shadow-sm">
                        <svg class="w-6 h-6 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 7h6m0 10v-3m-3 3h.01M9 17h.01M9 14h.01M12 14h.01M15 11h.01M12 11h.01M9 11h.01M7 21h10a2 2 0 002-2V5a2 2 0 00-2-2H7a2 2 0 00-2 2v14a2 2 0 002 2z"></path>
                        </svg>
                    </div>
                    <div>
                        <h1 class="text-lg font-bold text-gray-900">"Mercastats"</h1>
                        <p class="text-xs text-gray-500">"Dashboard"</p>
                        {move || if is_demo_user.get() {
                            view! {
                                <span class="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-amber-100 text-amber-800 mt-1">
                                    "Modo Demo"
                                </span>
                            }.into_view()
                        } else {
                            view! {}.into_view()
                        }}
                    </div>
                </div>
            </div>

            // Navegación
            <nav class="flex-1 p-4 space-y-1">
                <p class="px-3 py-2 text-xs font-semibold text-gray-400 uppercase tracking-wider">
                    "Menú"
                </p>

                // Opción: Upload
                <button
                    class=move || {
                        let base = "w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium transition-all";
                        if current_view.get() == DashboardView::Upload {
                            format!("{} bg-primary-50 text-primary-700", base)
                        } else {
                            format!("{} text-gray-700 hover:bg-gray-100 hover:text-gray-900", base)
                        }
                    }
                    on:click=move |_| on_view_change.call(DashboardView::Upload)
                >
                    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12"></path>
                    </svg>
                    <span>"Subir ticket"</span>
                    {move || if current_view.get() == DashboardView::Upload {
                        view! {
                            <div class="ml-auto w-1.5 h-1.5 bg-primary-600 rounded-full"></div>
                        }.into_view()
                    } else {
                        view! {}.into_view()
                    }}
                </button>

                // Opción: Histórico
                <button
                    class=move || {
                        let base = "w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium transition-all";
                        if current_view.get() == DashboardView::History {
                            format!("{} bg-primary-50 text-primary-700", base)
                        } else {
                            format!("{} text-gray-700 hover:bg-gray-100 hover:text-gray-900", base)
                        }
                    }
                    on:click=move |_| on_view_change.call(DashboardView::History)
                >
                    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2"></path>
                    </svg>
                    <span>"Histórico"</span>
                    {move || if current_view.get() == DashboardView::History {
                        view! {
                            <div class="ml-auto w-1.5 h-1.5 bg-primary-600 rounded-full"></div>
                        }.into_view()
                    } else {
                        view! {}.into_view()
                    }}
                </button>

                // Opción: Estadísticas
                <button
                    class=move || {
                        let base = "w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium transition-all";
                        if current_view.get() == DashboardView::Stats {
                            format!("{} bg-primary-50 text-primary-700", base)
                        } else {
                            format!("{} text-gray-700 hover:bg-gray-100 hover:text-gray-900", base)
                        }
                    }
                    on:click=move |_| on_view_change.call(DashboardView::Stats)
                >
                    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z"></path>
                    </svg>
                    <span>"Estadísticas"</span>
                    {move || if current_view.get() == DashboardView::Stats {
                        view! {
                            <div class="ml-auto w-1.5 h-1.5 bg-primary-600 rounded-full"></div>
                        }.into_view()
                    } else {
                        view! {}.into_view()
                    }}
                </button>

                // Opción: Evolución mensual
                <button
                    class=move || {
                        let base = "w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium transition-all";
                        if current_view.get() == DashboardView::MonthlyEvolution {
                            format!("{} bg-primary-50 text-primary-700", base)
                        } else {
                            format!("{} text-gray-700 hover:bg-gray-100 hover:text-gray-900", base)
                        }
                    }
                    on:click=move |_| on_view_change.call(DashboardView::MonthlyEvolution)
                >
                    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 17l6-6 4 4 8-8"></path>
                    </svg>
                    <span>"Evolución mensual"</span>
                    {move || if current_view.get() == DashboardView::MonthlyEvolution {
                        view! {
                            <div class="ml-auto w-1.5 h-1.5 bg-primary-600 rounded-full"></div>
                        }.into_view()
                    } else {
                        view! {}.into_view()
                    }}
                </button>

                // Opción: Predicción Next Shop
                <button
                    class=move || {
                        let base = "w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium transition-all";
                        if current_view.get() == DashboardView::Prediction {
                            format!("{} bg-primary-50 text-primary-700", base)
                        } else {
                            format!("{} text-gray-700 hover:bg-gray-100 hover:text-gray-900", base)
                        }
                    }
                    on:click=move |_| on_view_change.call(DashboardView::Prediction)
                >
                    <svg
                        class="w-5 h-5 flex-shrink-0"
                        viewBox="0 0 24 24"
                        aria-hidden="true"
                    >
                        <path
                            fill="currentColor"
                            fill-rule="evenodd"
                            clip-rule="evenodd"
                            d="M9.813 3.172a.75.75 0 0 1 1.374 0l1.471 3.513 3.513 1.471a.75.75 0 0 1 0 1.374l-3.513 1.471-1.471 3.513a.75.75 0 0 1-1.374 0l-1.47-3.513-3.514-1.471a.75.75 0 0 1 0-1.374l3.513-1.471 1.471-3.513Zm7.137 5.878a.75.75 0 0 1 1.348 0l.68 1.62 1.62.679a.75.75 0 0 1 0 1.393l-1.62.68-.68 1.62a.75.75 0 0 1-1.393 0l-.68-1.62-1.62-.68a.75.75 0 0 1 0-1.392l1.62-.68.68-1.62Zm-12.95 4.99a.75.75 0 0 1 1.066-.223l.402.268.268.402a.75.75 0 1 1-1.289.844l-.268-.402-.402-.268a.75.75 0 0 1-.223-1.066Z"
                        />
                    </svg>
                    <span>"Predicción AI"</span>
                    {move || if current_view.get() == DashboardView::Prediction {
                        view! {
                            <div class="ml-auto w-1.5 h-1.5 bg-primary-600 rounded-full"></div>
                        }.into_view()
                    } else {
                        view! {}.into_view()
                    }}
                </button>
            </nav>

            // Footer con usuario y logout
            <div class="p-4 border-t border-gray-200">
                <div class="flex items-center gap-3 px-3 py-2">
                    <div class="flex items-center justify-center w-8 h-8 bg-gray-200 rounded-full">
                        <svg class="w-5 h-5 text-gray-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z"></path>
                        </svg>
                    </div>
                    <div class="flex-1 min-w-0">
                        <p class="text-sm font-medium text-gray-900 truncate">
                            {move || user_email.get()}
                        </p>
                        <p class="text-xs text-gray-500">"Usuario"</p>
                    </div>
                    <button
                        class="p-1.5 text-gray-400 hover:text-gray-600 hover:bg-gray-100 rounded-lg transition-colors"
                        on:click=move |_| {
                            // Limpiar localStorage
                            if let Some(window) = web_sys::window() {
                                if let Ok(Some(storage)) = window.local_storage() {
                                    let _ = storage.remove_item("auth_token");
                                    let _ = storage.remove_item("user_email");
                                    let _ = storage.remove_item("user_is_demo");
                                }
                                // Redirigir al login
                                let _ = window.location().set_href("/");
                            }
                        }
                        title="Cerrar sesión"
                    >
                        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1"></path>
                        </svg>
                    </button>
                </div>
            </div>
        </aside>
    }
}
