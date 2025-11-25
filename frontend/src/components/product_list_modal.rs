use leptos::*;
use crate::api::stats::{get_all_products, TopProductItem};
use crate::components::{Chart, ChartSeriesData, ChartType};

/// Height in pixels allocated for each product bar in the horizontal bar chart.
/// This ensures adequate spacing for product labels and bar visibility.
const PIXELS_PER_PRODUCT: u64 = 40;

#[component]
pub fn ProductListModal(
    is_open: Signal<bool>,
    on_close: Callback<()>,
    sort_by: Signal<String>, // "quantity" or "spending"
    title: Signal<String>,
) -> impl IntoView {
    let (products, set_products) = create_signal(Vec::<TopProductItem>::new());
    let (loading, set_loading) = create_signal(false);

    // Fetch products when modal opens or sort_by changes
    create_effect(move |_| {
        if is_open.get() {
            set_loading.set(true);
            let sort = sort_by.get();
            spawn_local(async move {
                match get_all_products(&sort, 100).await {
                    Ok(data) => set_products.set(data),
                    Err(e) => leptos::logging::error!("Error fetching products: {}", e),
                }
                set_loading.set(false);
            });
        }
    });

    let chart_data = move || {
        let data = products.get();
        if data.is_empty() {
            return None;
        }

        let product_names: Vec<String> = data.iter().map(|p| p.nombre.clone()).collect();
        let product_values: Vec<f64> = data
            .iter()
            .map(|p| {
                if sort_by.get() == "spending" {
                    p.gasto_total
                        .clone()
                        .unwrap_or_else(|| "0".to_string())
                        .parse::<f64>()
                        .unwrap_or(0.0)
                } else {
                    p.cantidad_total.unwrap_or(0) as f64
                }
            })
            .collect();

        let series_name = if sort_by.get() == "spending" {
            "Gasto Total (€)".to_string()
        } else {
            "Cantidad Total".to_string()
        };

        let series_data = ChartSeriesData {
            name: series_name,
            data: product_values,
        };

        // Calculate height based on number of products
        let height = std::cmp::max(400, data.len() as u64 * PIXELS_PER_PRODUCT);

        Some((series_data, product_names, height))
    };

    view! {
        <Show when=move || is_open.get()>
            <div class="fixed inset-0 z-50 flex items-center justify-center overflow-y-auto overflow-x-hidden bg-gray-900 bg-opacity-50 backdrop-blur-sm transition-opacity duration-300">
                <div class="relative w-full max-w-4xl max-h-[90vh] mx-4 bg-white rounded-xl shadow-2xl flex flex-col animate-scale-in">
                    // Header
                    <div class="flex items-center justify-between p-6 border-b border-gray-100">
                        <h3 class="text-xl font-bold text-gray-900">
                            {move || title.get()}
                        </h3>
                        <button
                            on:click=move |_| on_close.call(())
                            class="text-gray-400 hover:text-gray-500 transition-colors focus:outline-none"
                        >
                            <svg class="h-6 w-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                            </svg>
                        </button>
                    </div>

                    // Content
                    <div class="flex-1 overflow-y-auto p-6 custom-scrollbar">
                        <Show
                            when=move || !loading.get()
                            fallback=move || view! {
                                <div class="flex items-center justify-center h-64">
                                    <div class="animate-spin h-10 w-10 border-4 border-primary-500 border-t-transparent rounded-full"></div>
                                </div>
                            }
                        >
                            {move || match chart_data() {
                                Some((series, categories, height)) => view! {
                                    <div style=format!("height: {}px", height)>
                                        <Chart
                                            id=format!("modal-chart-{}", sort_by.get())
                                            chart_type=ChartType::BarHorizontal
                                            series=vec![series]
                                            categories=categories
                                            height=height as i32
                                            title="".to_string()
                                        />
                                    </div>
                                }.into_view(),
                                None => view! {
                                    <div class="text-center text-gray-500 py-10">
                                        "No hay datos disponibles"
                                    </div>
                                }.into_view()
                            }}
                        </Show>
                    </div>
                </div>
            </div>
        </Show>
    }
}
