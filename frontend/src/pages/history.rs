use crate::api::tickets::{get_user_ticket_history, TicketHistoryResponse};
use crate::components::{Card};
use leptos::*;

#[component]
pub fn TicketHistory() -> impl IntoView {
    let (history_data, set_history_data) = create_signal(None::<TicketHistoryResponse>);
    let (loading, set_loading) = create_signal(true);
    let (error_message, set_error_message) = create_signal(None::<String>);

    // Cargar histórico al montar el componente
    create_effect(move |_| {
        // Obtener email del usuario
        if let Some(window) = web_sys::window() {
            if let Ok(Some(storage)) = window.local_storage() {
                if let Ok(Some(email)) = storage.get_item("user_email") {
                    set_loading.set(true);
                    set_error_message.set(None);

                    spawn_local(async move {
                        match get_user_ticket_history(&email).await {
                            Ok(data) => {
                                set_history_data.set(Some(data));
                                set_loading.set(false);
                            }
                            Err(err) => {
                                set_error_message.set(Some(err));
                                set_loading.set(false);
                            }
                        }
                    });
                } else {
                    set_error_message.set(Some(
                        "No se encontró el email del usuario".to_string(),
                    ));
                    set_loading.set(false);
                }
            }
        }
    });

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
                                {data.stats.total_gastado.clone().unwrap_or_else(|| "0.00".to_string())}"€"
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
                                {data.stats.productos_unicos.unwrap_or(0)}
                            </div>
                            <div class="text-sm text-gray-600">"Productos únicos"</div>
                        </div>
                    </Card>
                </div>

                // Lista de tickets
                <Card class="animate-slide-up".to_string()>
                    <div class="space-y-4">
                        <h2 class="text-xl font-bold text-gray-900 flex items-center">
                            <svg class="w-6 h-6 mr-2 text-primary-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2"></path>
                            </svg>
                            {format!("Todos los tickets ({})", data.tickets.len())}
                        </h2>

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
                            // Clonar los tickets fuera del closure para evitar problemas de lifetime
                            let tickets_clone = data.tickets.clone();
                            view! {
                                <div class="space-y-3">
                                    {tickets_clone.into_iter().map(|ticket| {
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
                                    }).collect_view()}
                                </div>
                            }.into_view()
                        }}
                    </div>
                </Card>
            })}
        </div>
    }
}
