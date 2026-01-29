use crate::api::stats::*;
use crate::components::{Chart, ChartSeriesData, ChartType, KpiCard, ProductListModal};
use leptos::*;

#[component]
pub fn Stats() -> impl IntoView {
    let stats = create_resource(
        || (),
        |_| async {
            get_dashboard_stats()
                .await
                .map_err(|e| leptos::logging::error!("Error cargando estadísticas: {}", e))
        },
    );

    let (modal_open, set_modal_open) = create_signal(false);
    let (modal_sort_by, set_modal_sort_by) = create_signal("quantity".to_string());
    let (modal_title, set_modal_title) = create_signal("".to_string());

    let open_modal = move |sort: String, title: String| {
        set_modal_sort_by.set(sort);
        set_modal_title.set(title);
        set_modal_open.set(true);
    };

    view! {
        <div class="min-h-screen bg-gradient-to-br from-gray-50 to-gray-100 p-4 sm:p-6 lg:p-8">
            <div class="max-w-7xl mx-auto">
                {/* Header */}
                <div class="mb-8 animate-slide-down">
                    <h1 class="text-4xl font-bold text-gray-900 mb-2">
                        "📊 Tu Dashboard Estadístico"
                    </h1>
                    <p class="text-gray-600">
                        "Analiza tus hábitos de compra y tendencias de gasto"
                    </p>
                </div>

                {/* Loading State */}
                <Suspense fallback=move || {
                    view! {
                        <div class="flex items-center justify-center h-96">
                            <div class="animate-spin">
                                <div class="h-12 w-12 border-4 border-primary-500 border-t-transparent rounded-full"></div>
                            </div>
                        </div>
                    }
                }>
                    {move || {
                        match stats.get() {
                            Some(Ok(data)) => {
                                view! {
                                    <div class="space-y-6">
                                        {/* KPI Cards - Row 1 */}
                                        <div class="grid grid-cols-1 md:grid-cols-3 gap-6">
                                            <KpiCard
                                                title="Gasto Mes Actual".to_string()
                                                value=parse_decimal(&data.current_month_spend).unwrap_or(0.0)
                                                decimals=2
                                                prefix="€".to_string()
                                                trend=Some(data.trend_percentage)
                                                icon="💰".to_string()
                                                animation_delay=0
                                            />

                                            <KpiCard
                                                title="Ticket Promedio".to_string()
                                                value=parse_decimal(&data.average_spending_per_ticket.clone().unwrap_or_default()).unwrap_or(0.0)
                                                decimals=2
                                                prefix="€".to_string()
                                                icon="🛒".to_string()
                                                animation_delay=100
                                            />

                                            <KpiCard
                                                title="Productos Únicos".to_string()
                                                value=data.unique_products.unwrap_or(0) as f64
                                                decimals=0
                                                suffix="".to_string()
                                                icon="📦".to_string()
                                                animation_delay=200
                                            />
                                        </div>

                                        {/* Main Chart - Tendencia */}
                                        <div class="animate-fade-in delay-200">
                                            <TendenciaChart daily_data=data.daily_spending_trend.clone() />
                                        </div>

                                        {/* Two Column Layout - Top Products */}
                                        <div class="grid grid-cols-1 lg:grid-cols-2 gap-6 animate-fade-in delay-300">
                                            <TopProductsChart
                                                title="Top Productos (por cantidad)".to_string()
                                                products=data.top_products_quantity.clone()
                                                on_view_all=move || open_modal("quantity".to_string(), "Top Productos (por cantidad)".to_string())
                                            />

                                            <TopProductsChart
                                                title="Top Productos (por gasto)".to_string()
                                                products=data.top_products_spending.clone()
                                                on_view_all=move || open_modal("spending".to_string(), "Top Productos (por gasto)".to_string())
                                            />
                                        </div>

                                        {/* Distribution Charts */}
                                        <div class="grid grid-cols-1 lg:grid-cols-2 gap-6 animate-fade-in delay-400">
                                            <DistributionChart
                                                title="Distribución por Día de la Semana".to_string()
                                                data=data.weekly_distribution.clone()
                                            />

                                            <DistributionChart
                                                title="Distribución por Hora del Día".to_string()
                                                data=data.hourly_distribution.clone()
                                            />
                                        </div>

                                        {/* Stats Summary */}
                                        <div class="bg-white rounded-lg border border-gray-100 p-6 shadow-sm animate-fade-in delay-500">
                                            <h3 class="text-lg font-semibold text-gray-900 mb-4">
                                                "📈 Resumen General"
                                            </h3>

                                            <div class="grid grid-cols-1 md:grid-cols-2 gap-8">
                                                <div>
                                                    <p class="text-sm text-gray-600 mb-2">"Total de Tickets"</p>
                                                    <p class="text-3xl font-bold text-gray-900">
                                                        {data.total_tickets.unwrap_or(0)}
                                                    </p>
                                                </div>

                                                <div>
                                                    <p class="text-sm text-gray-600 mb-2">"Gasto Mes Anterior"</p>
                                                    <p class="text-3xl font-bold text-gray-900">
                                                        "€" {data.previous_month_spend.clone()}
                                                    </p>
                                                </div>
                                            </div>
                                        </div>
                                    </div>
                                }.into_view()
                            }
                            Some(Err(_)) => {
                                view! {
                                    <div class="bg-red-50 border border-red-200 rounded-lg p-6 text-center">
                                        <h3 class="text-lg font-semibold text-red-900 mb-2">
                                            "⚠️ Error"
                                        </h3>
                                        <p class="text-red-700">
                                            "No se pudieron cargar las estadísticas. Intenta nuevamente más tarde."
                                        </p>
                                    </div>
                                }.into_view()
                            }
                            None => {
                                view! {
                                    <div class="flex items-center justify-center h-96">
                                        <div class="animate-spin">
                                            <div class="h-12 w-12 border-4 border-primary-500 border-t-transparent rounded-full"></div>
                                        </div>
                                    </div>
                                }.into_view()
                            }
                        }
                    }}
                </Suspense>
            </div>

            <ProductListModal
                is_open=modal_open.into()
                on_close=Callback::from(move |_| set_modal_open.set(false))
                sort_by=modal_sort_by.into()
                title=modal_title.into()
            />
        </div>
    }
}

#[component]
fn TendenciaChart(daily_data: Vec<DailySpendPoint>) -> impl IntoView {
    let series_data = ChartSeriesData {
        name: "Gasto Diario".to_string(),
        data: daily_data
            .iter()
            .map(|p| parse_decimal(&p.total).unwrap_or(0.0))
            .collect(),
    };

    let categories: Vec<String> = daily_data.iter().map(|p| p.fecha.clone()).collect();
    let has_no_data = daily_data.is_empty();
    let has_single_point = daily_data.len() == 1;
    let single_point = daily_data.first().cloned();
    let chart_id = format!("tendencia-chart-{}", daily_data.len());

    view! {
        <div class="bg-white rounded-lg border border-gray-100 p-6 shadow-sm">
            <h2 class="text-lg font-semibold text-gray-900 mb-4">
                "📈 Tendencia de Gasto (Últimos 30 días)"
            </h2>

            {if has_no_data {
                view! {
                    <div class="flex items-center justify-center h-64 text-gray-500">
                        <div class="text-center space-y-1">
                            <p class="text-base font-semibold text-gray-900">"Aún no hay datos para mostrar"</p>
                            <p class="text-sm">"Sube un ticket para ver tu tendencia de gasto"</p>
                        </div>
                    </div>
                }.into_view()
            } else if has_single_point {
                let point = single_point.unwrap();
                let value = parse_decimal(&point.total).unwrap_or(0.0);
                view! {
                    <div class="space-y-3">
                        <div class="rounded-lg border border-blue-100 bg-blue-50/70 text-blue-800 px-3 py-2 text-sm font-medium">
                            "Solo hay una compra en los £ltimos 30 d¡as. Mostramos el valor como barra hasta tener m s puntos."
                        </div>
                        <Chart
                            id=chart_id.clone()
                            chart_type=ChartType::BarHorizontal
                            series=vec![ChartSeriesData {
                                name: "Gasto Diario".to_string(),
                                data: vec![value],
                            }]
                            categories=vec![point.fecha]
                            height=260
                            title="".to_string()
                        />
                    </div>
                }.into_view()
            } else {
                view! {
                    <Chart
                        id=chart_id
                        chart_type=ChartType::Area
                        series=vec![series_data]
                        categories=categories
                        height=400
                        title="".to_string()
                    />
                }.into_view()
            }}
        </div>
    }
}
#[component]
fn TopProductsChart<F>(
    title: String,
    products: Vec<TopProductItem>,
    on_view_all: F,
) -> impl IntoView
where
    F: Fn() + 'static,
{
    let product_names: Vec<String> = products.iter().map(|p| p.nombre.clone()).collect();
    let product_values: Vec<f64> = products
        .iter()
        .map(|p| {
            parse_decimal(&p.gasto_total.clone().unwrap_or_else(|| "0".to_string())).unwrap_or(0.0)
        })
        .collect();

    let series_data = ChartSeriesData {
        name: "Gasto Total (€)".to_string(),
        data: product_values,
    };

    let chart_id = format!("products-{}", title.replace(" ", "-"));
    let title_clone = title.clone();

    view! {
        <div class="bg-white rounded-lg border border-gray-100 p-6 shadow-sm">
            <div class="flex items-center justify-between mb-4">
                <h2 class="text-lg font-semibold text-gray-900">
                    {title_clone}
                </h2>
                <button
                    on:click=move |_| on_view_all()
                    class="text-sm text-primary-600 hover:text-primary-700 font-medium hover:underline transition-colors"
                >
                    "Ver todos"
                </button>
            </div>

            <Chart
                id=chart_id
                chart_type=ChartType::BarHorizontal
                series=vec![series_data]
                categories=product_names
                height=350
                title="".to_string()
            />
        </div>
    }
}

#[component]
fn DistributionChart(title: String, data: Vec<TimeDistributionPoint>) -> impl IntoView {
    let labels: Vec<String> = data.iter().map(|p| p.tiempo.clone()).collect();
    let values: Vec<f64> = data
        .iter()
        .map(|p| parse_decimal(&p.total).unwrap_or(0.0))
        .collect();

    let series_data = ChartSeriesData {
        name: "Gasto (€)".to_string(),
        data: values,
    };

    let chart_id = format!("distribution-{}", title.replace(" ", "-"));
    let title_clone = title.clone();

    view! {
        <div class="bg-white rounded-lg border border-gray-100 p-6 shadow-sm">
            <h2 class="text-lg font-semibold text-gray-900 mb-4">
                {title_clone}
            </h2>

            <Chart
                id=chart_id
                chart_type=ChartType::Bar
                series=vec![series_data]
                categories=labels
                height=300
                title="".to_string()
            />
        </div>
    }
}

/// Helper para parsear strings decimales
fn parse_decimal(s: &str) -> Option<f64> {
    s.parse::<f64>().ok()
}
