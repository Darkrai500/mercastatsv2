use crate::api::tickets::{get_user_ticket_history, TicketHistoryResponse};
use crate::components::Card;
use leptos::*;
use leptos_router::use_navigate;

#[derive(Debug, Clone, Copy, PartialEq)]
enum SortBy {
    Date,
    Price,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SortOrder {
    Ascending,
    Descending,
}

#[component]
pub fn TicketHistory() -> impl IntoView {
    let (history_data, set_history_data) = create_signal(None::<TicketHistoryResponse>);
    let (loading, set_loading) = create_signal(true);
    let (error_message, set_error_message) = create_signal(None::<String>);
    let (sort_by, set_sort_by) = create_signal(SortBy::Date);
    let (sort_order, set_sort_order) = create_signal(SortOrder::Descending);
    let (dropdown_open, set_dropdown_open) = create_signal(false);

    // Cargar historico al montar el componente
    let navigate = use_navigate();

    create_effect(move |_| {
        set_loading.set(true);
        set_error_message.set(None);

        let navigate = navigate.clone();
        let set_history_data = set_history_data;
        let set_loading = set_loading;
        let set_error_message = set_error_message;

        spawn_local(async move {
            let result = get_user_ticket_history().await;
            set_loading.set(false);

            match result {
                Ok(data) => set_history_data.set(Some(data)),
                Err(err) => {
                    if err.contains("sesion") {
                        navigate("/", Default::default());
                    }
                    set_error_message.set(Some(err));
                }
            }
        });
    });

    // Función para ordenar tickets
    let sorted_tickets = move || {
        history_data.get().map(|data| {
            let mut tickets = data.tickets.clone();

            match (sort_by.get(), sort_order.get()) {
                (SortBy::Date, SortOrder::Ascending) => {
                    tickets.sort_by(|a, b| a.fecha_hora.cmp(&b.fecha_hora));
                }
                (SortBy::Date, SortOrder::Descending) => {
                    tickets.sort_by(|a, b| b.fecha_hora.cmp(&a.fecha_hora));
                }
                (SortBy::Price, SortOrder::Ascending) => {
                    tickets.sort_by(|a, b| {
                        let price_a: f64 = a.total.parse().unwrap_or(0.0);
                        let price_b: f64 = b.total.parse().unwrap_or(0.0);
                        price_a.partial_cmp(&price_b).unwrap_or(std::cmp::Ordering::Equal)
                    });
                }
                (SortBy::Price, SortOrder::Descending) => {
                    tickets.sort_by(|a, b| {
                        let price_a: f64 = a.total.parse().unwrap_or(0.0);
                        let price_b: f64 = b.total.parse().unwrap_or(0.0);
                        price_b.partial_cmp(&price_a).unwrap_or(std::cmp::Ordering::Equal)
                    });
                }
            }

            tickets
        })
    };

    view! {
        <div class="space-y-6">
            // Header
            <div>
                <h1 class="text-3xl font-bold text-gray-900 mb-2">
                    "Histórico de Tickets"
                </h1>
                <p class="text-gray-600">
                    "Revisa todos tus tickets de compra guardados"
                </p>
            </div>

            // Error message
            {move || error_message.get().map(|msg| view! {
                <div class="p-4 bg-red-50 border border-red-200 rounded-xl animate-slide-up">
                    <div class="flex items-center">
                        <svg class="w-6 h-6 text-red-600 mr-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path>
                        </svg>
                        <p class="text-red-800 font-medium">{msg}</p>
                    </div>
                </div>
            })}

            // Loading
            {move || loading.get().then(|| view! {
                <Card class="animate-slide-up".to_string()>
                    <div class="flex flex-col items-center justify-center py-12">
                        <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-primary-600 mb-4"></div>
                        <p class="text-gray-600">"Cargando histórico de tickets..."</p>
                    </div>
                </Card>
            })}

            // Estadísticas
            {move || history_data.get().map(|data| view! {
                <div class="grid grid-cols-1 md:grid-cols-3 gap-6 animate-slide-up">
                    <Card padding=true>
                        <div class="text-center">
                            <div class="inline-flex items-center justify-center w-12 h-12 bg-primary-100 rounded-full mb-3">
                                <svg class="w-6 h-6 text-primary-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"></path>
                                </svg>
                            </div>
                            <div class="text-3xl font-bold text-gray-900 mb-1">
                                {data.stats.total_tickets.unwrap_or(0)}
                            </div>
                            <div class="text-sm text-gray-600">"Tickets guardados"</div>
                        </div>
                    </Card>

                    <Card padding=true>
                        <div class="text-center">
                            <div class="inline-flex items-center justify-center w-12 h-12 bg-accent-100 rounded-full mb-3">
                                <svg class="w-6 h-6 text-accent-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8c-1.657 0-3 .895-3 2s1.343 2 3 2 3 .895 3 2-1.343 2-3 2m0-8c1.11 0 2.08.402 2.599 1M12 8V7m0 1v8m0 0v1m0-1c-1.11 0-2.08-.402-2.599-1M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path>
                                </svg>
                            </div>
                            <div class="text-3xl font-bold text-gray-900 mb-1">
                                {format!("{:.2}€", data.stats.gasto_total.unwrap_or(0.0))}
                            </div>
                            <div class="text-sm text-gray-600">"Gasto total"</div>
                        </div>
                    </Card>

                    <Card padding=true>
                        <div class="text-center">
                            <div class="inline-flex items-center justify-center w-12 h-12 bg-green-100 rounded-full mb-3">
                                <svg class="w-6 h-6 text-green-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 3h2l.4 2M7 13h10l4-8H5.4M7 13L5.4 5M7 13l-2.293 2.293c-.63.63-.184 1.707.707 1.707H17m0 0a2 2 0 100 4 2 2 0 000-4zm-8 2a2 2 0 11-4 0 2 2 0 014 0z"></path>
                                </svg>
                            </div>
                            <div class="text-3xl font-bold text-gray-900 mb-1">
                                {format!("{:.2}€", data.stats.gasto_promedio.unwrap_or(0.0))}
                            </div>
                            <div class="text-sm text-gray-600">"Gasto promedio"</div>
                            <div class="text-xs text-gray-500 mt-1">
                                {format!(
                                    "Ultimo ticket: {}",
                                    data.stats
                                        .ultimo_ticket
                                        .clone()
                                        .unwrap_or_else(|| "Sin registros".to_string())
                                )}
                            </div>
                        </div>
                    </Card>
                </div>

                // Lista de tickets
                <Card class="animate-slide-up".to_string()>
                    <div class="space-y-4">
                        <div class="flex items-center justify-between">
                            <h2 class="text-xl font-bold text-gray-900 flex items-center">
                                <svg class="w-6 h-6 mr-2 text-primary-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2"></path>
                                </svg>
                                {format!("Todos los tickets ({})", data.tickets.len())}
                            </h2>

                            // Selector de ordenamiento
                            <div class="relative">
                                <button
                                    on:click=move |_| set_dropdown_open.update(|v| *v = !*v)
                                    class="flex items-center space-x-2 px-4 py-2 bg-white border border-gray-300 rounded-lg hover:bg-gray-50 transition-colors focus:outline-none focus:ring-2 focus:ring-primary-500 focus:border-transparent"
                                >
                                    <svg class="w-5 h-5 text-gray-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 4h13M3 8h9m-9 4h6m4 0l4-4m0 0l4 4m-4-4v12"></path>
                                    </svg>
                                    <span class="text-sm font-medium text-gray-700">
                                        {move || match sort_by.get() {
                                            SortBy::Date => "Fecha",
                                            SortBy::Price => "Precio",
                                        }}
                                    </span>
                                    <svg
                                        class=move || format!(
                                            "w-4 h-4 text-gray-500 transition-transform {}",
                                            if dropdown_open.get() { "rotate-180" } else { "" }
                                        )
                                        fill="none"
                                        stroke="currentColor"
                                        viewBox="0 0 24 24"
                                    >
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"></path>
                                    </svg>
                                </button>

                                // Dropdown menu
                                <div
                                    class=move || format!(
                                        "absolute right-0 mt-2 w-48 bg-white rounded-lg shadow-lg border border-gray-200 z-10 transition-all duration-200 {}",
                                        if dropdown_open.get() {
                                            "opacity-100 scale-100"
                                        } else {
                                            "opacity-0 scale-95 pointer-events-none"
                                        }
                                    )
                                >
                                    <div class="py-1">
                                        // Ordenar por Fecha
                                        <div class="px-2 py-1.5 text-xs font-semibold text-gray-500 uppercase tracking-wider">
                                            "Ordenar por"
                                        </div>
                                        <button
                                            on:click=move |_| {
                                                set_sort_by.set(SortBy::Date);
                                                set_sort_order.set(SortOrder::Descending);
                                                set_dropdown_open.set(false);
                                            }
                                            class=move || format!(
                                                "w-full text-left px-4 py-2 text-sm hover:bg-gray-50 transition-colors flex items-center justify-between {}",
                                                if sort_by.get() == SortBy::Date && sort_order.get() == SortOrder::Descending {
                                                    "bg-primary-50 text-primary-700 font-medium"
                                                } else {
                                                    "text-gray-700"
                                                }
                                            )
                                        >
                                            <span>"Fecha (más reciente)"</span>
                                            {move || (sort_by.get() == SortBy::Date && sort_order.get() == SortOrder::Descending).then(|| view! {
                                                <svg class="w-4 h-4 text-primary-600" fill="currentColor" viewBox="0 0 20 20">
                                                    <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"></path>
                                                </svg>
                                            })}
                                        </button>
                                        <button
                                            on:click=move |_| {
                                                set_sort_by.set(SortBy::Date);
                                                set_sort_order.set(SortOrder::Ascending);
                                                set_dropdown_open.set(false);
                                            }
                                            class=move || format!(
                                                "w-full text-left px-4 py-2 text-sm hover:bg-gray-50 transition-colors flex items-center justify-between {}",
                                                if sort_by.get() == SortBy::Date && sort_order.get() == SortOrder::Ascending {
                                                    "bg-primary-50 text-primary-700 font-medium"
                                                } else {
                                                    "text-gray-700"
                                                }
                                            )
                                        >
                                            <span>"Fecha (más antigua)"</span>
                                            {move || (sort_by.get() == SortBy::Date && sort_order.get() == SortOrder::Ascending).then(|| view! {
                                                <svg class="w-4 h-4 text-primary-600" fill="currentColor" viewBox="0 0 20 20">
                                                    <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"></path>
                                                </svg>
                                            })}
                                        </button>

                                        <div class="border-t border-gray-200 my-1"></div>

                                        // Ordenar por Precio
                                        <button
                                            on:click=move |_| {
                                                set_sort_by.set(SortBy::Price);
                                                set_sort_order.set(SortOrder::Descending);
                                                set_dropdown_open.set(false);
                                            }
                                            class=move || format!(
                                                "w-full text-left px-4 py-2 text-sm hover:bg-gray-50 transition-colors flex items-center justify-between {}",
                                                if sort_by.get() == SortBy::Price && sort_order.get() == SortOrder::Descending {
                                                    "bg-primary-50 text-primary-700 font-medium"
                                                } else {
                                                    "text-gray-700"
                                                }
                                            )
                                        >
                                            <span>"Precio (mayor a menor)"</span>
                                            {move || (sort_by.get() == SortBy::Price && sort_order.get() == SortOrder::Descending).then(|| view! {
                                                <svg class="w-4 h-4 text-primary-600" fill="currentColor" viewBox="0 0 20 20">
                                                    <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"></path>
                                                </svg>
                                            })}
                                        </button>
                                        <button
                                            on:click=move |_| {
                                                set_sort_by.set(SortBy::Price);
                                                set_sort_order.set(SortOrder::Ascending);
                                                set_dropdown_open.set(false);
                                            }
                                            class=move || format!(
                                                "w-full text-left px-4 py-2 text-sm hover:bg-gray-50 transition-colors flex items-center justify-between {}",
                                                if sort_by.get() == SortBy::Price && sort_order.get() == SortOrder::Ascending {
                                                    "bg-primary-50 text-primary-700 font-medium"
                                                } else {
                                                    "text-gray-700"
                                                }
                                            )
                                        >
                                            <span>"Precio (menor a mayor)"</span>
                                            {move || (sort_by.get() == SortBy::Price && sort_order.get() == SortOrder::Ascending).then(|| view! {
                                                <svg class="w-4 h-4 text-primary-600" fill="currentColor" viewBox="0 0 20 20">
                                                    <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"></path>
                                                </svg>
                                            })}
                                        </button>
                                    </div>
                                </div>
                            </div>
                        </div>

                        {if data.tickets.is_empty() {
                            view! {
                                <div class="text-center py-12">
                                    <div class="inline-flex items-center justify-center w-16 h-16 bg-gray-100 rounded-full mb-4">
                                        <svg class="w-8 h-8 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"></path>
                                        </svg>
                                    </div>
                                    <h3 class="text-lg font-medium text-gray-900 mb-1">"No hay tickets todavía"</h3>
                                    <p class="text-gray-600 mb-4">"Sube tu primer ticket para empezar a analizar tus compras"</p>
                                    <a
                                        href="/dashboard"
                                        class="inline-flex items-center justify-center px-4 py-2 bg-primary-600 text-white rounded-lg font-medium hover:bg-primary-700 transition-colors"
                                    >
                                        "Subir ticket"
                                    </a>
                                </div>
                            }.into_view()
                        } else {
                            view! {
                                <div class="space-y-3">
                                    {move || {
                                        // Obtener y ordenar tickets de forma reactiva
                                        let tickets = sorted_tickets().unwrap_or_else(Vec::new);

                                        tickets.into_iter().map(|ticket| {
                                            let numero_factura = ticket.numero_factura.clone();
                                            let fecha_str = ticket.fecha_hora.split('T').next().unwrap_or(&ticket.fecha_hora).to_string();
                                            let tienda_str = ticket.tienda.clone().unwrap_or_else(|| "N/A".to_string());
                                            let ubicacion_opt = ticket.ubicacion.clone();
                                            let total_str = ticket.total.clone();
                                            let num_productos_str = ticket.num_productos.map(|n| format!("{} productos", n)).unwrap_or_else(|| "N/A".to_string());

                                            view! {
                                                <div class="p-4 bg-gray-50 hover:bg-gray-100 rounded-lg transition-colors border border-gray-200">
                                                    <div class="flex items-center justify-between">
                                                        <div class="flex-1 grid grid-cols-1 md:grid-cols-4 gap-4">
                                                            // Factura
                                                            <div>
                                                                <div class="text-xs text-gray-500 mb-1">"Factura"</div>
                                                                <div class="font-semibold text-gray-900">{numero_factura}</div>
                                                            </div>

                                                            // Fecha
                                                            <div>
                                                                <div class="text-xs text-gray-500 mb-1">"Fecha"</div>
                                                                <div class="text-gray-900">{fecha_str}</div>
                                                            </div>

                                                            // Tienda
                                                            <div>
                                                                <div class="text-xs text-gray-500 mb-1">"Tienda"</div>
                                                                <div class="text-gray-900">{tienda_str}</div>
                                                                {ubicacion_opt.map(|ubicacion| view! {
                                                                    <div class="text-xs text-gray-500 mt-0.5">{ubicacion}</div>
                                                                })}
                                                            </div>

                                                            // Total
                                                            <div class="text-right">
                                                                <div class="text-xs text-gray-500 mb-1">"Total"</div>
                                                                <div class="text-xl font-bold text-primary-600">
                                                                    {total_str}"€"
                                                                </div>
                                                                <div class="text-xs text-gray-500">{num_productos_str}</div>
                                                            </div>
                                                        </div>
                                                    </div>
                                                </div>
                                            }
                                        }).collect_view()
                                    }}
                                </div>
                            }.into_view()
                        }}
                    </div>
                </Card>
            })}
        </div>
    }
}
