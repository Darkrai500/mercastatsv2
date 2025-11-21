use crate::api::stats::{get_monthly_evolution, MonthlyEvolutionResponse};
use crate::components::{Chart, ChartSeriesData, ChartType, KpiCard};
use leptos::*;

#[component]
pub fn MonthlyEvolution() -> impl IntoView {
    let (months_filter, set_months_filter) = create_signal(12u32);
    let (show_significant, set_show_significant) = create_signal(false);

    let monthly_data = create_resource(
        move || months_filter.get(),
        |months| async move {
            get_monthly_evolution(months)
                .await
                .map_err(|e| {
                    leptos::logging::error!("Error cargando evolucion mensual: {}", e);
                    e
                })
        },
    );

    view! {
        <div class="min-h-screen bg-gradient-to-br from-gray-50 via-white to-primary-50 p-4 sm:p-6 lg:p-8">
            <div class="max-w-7xl mx-auto space-y-6">
                <header class="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4 animate-slide-down">
                    <div>
                        <p class="uppercase tracking-[0.25rem] text-xs text-primary-500 font-semibold mb-2">
                            "Mercastats Pulse"
                        </p>
                        <h1 class="text-4xl font-bold text-gray-900">"Evolución mensual"</h1>
                        <p class="text-gray-600 mt-2">
                            "Tu gasto mes a mes con una lectura limpia, animada y comparativa."
                        </p>
                    </div>

                    <div class="flex flex-col sm:flex-row items-end sm:items-center gap-4">
                        <div class="flex items-center gap-2 bg-white/80 backdrop-blur px-2 py-2 rounded-2xl border border-gray-100 shadow-sm">
                            {[6u32, 12u32, 999u32].into_iter().map(|months| {
                                let active = move || months_filter.get() == months;
                                let button_class = move || {
                                    let base = "px-4 py-2 rounded-xl text-sm font-semibold transition-all duration-200";
                                    if active() {
                                        format!("{base} bg-primary-600 text-white shadow-md shadow-primary-200")
                                    } else {
                                        format!("{base} text-gray-600 hover:text-gray-900 hover:bg-gray-100")
                                    }
                                };
                                let label = if months == 999 { "All Time" } else { match months { 6 => "6M", 12 => "12M", _ => "" } };
                                let label_text = if label.is_empty() { format!("{}M", months) } else { label.to_string() };

                                view! {
                                    <button
                                        class=button_class
                                        on:click=move |_| set_months_filter.set(months)
                                    >
                                        {label_text}
                                    </button>
                                }
                            }).collect_view()}
                        </div>
                        
                        <label class="flex items-center gap-2 text-sm text-gray-600 cursor-pointer select-none bg-white/50 px-3 py-2 rounded-xl border border-transparent hover:border-gray-200 transition-all">
                            <input
                                type="checkbox"
                                class="rounded border-gray-300 text-primary-600 focus:ring-primary-500"
                                prop:checked=move || show_significant.get()
                                on:change=move |ev| set_show_significant.set(event_target_checked(&ev))
                            />
                            "Solo relevantes"
                        </label>
                    </div>
                </header>

                <Suspense fallback=move || view! { <MonthlySkeleton /> }>
                    {move || {
                        match monthly_data.get() {
                            Some(Ok(data)) => {
                                let payload = data.clone();
                                view! { <MonthlyContent data=payload months_filter=months_filter.get() show_significant=show_significant.get() /> }.into_view()
                            }
                            Some(Err(err)) => view! {
                                <div class="bg-red-50 border border-red-200 text-red-800 rounded-xl p-4 animate-fade-in">
                                    <p class="font-semibold mb-1">"No pudimos cargar la evolución mensual"</p>
                                    <p class="text-sm text-red-700">{err}</p>
                                </div>
                            }.into_view(),
                            None => view! { <MonthlySkeleton /> }.into_view(),
                        }
                    }}
                </Suspense>
            </div>
        </div>
    }
}

#[component]
fn MonthlyContent(data: MonthlyEvolutionResponse, months_filter: u32, show_significant: bool) -> impl IntoView {
    let current_total = parse_decimal(&data.current_month_total).unwrap_or(0.0);
    let previous_total = parse_decimal(&data.previous_month_total).unwrap_or(0.0);
    let average_monthly = parse_decimal(&data.average_monthly).unwrap_or(0.0);
    let year_to_date = parse_decimal(&data.year_to_date_total).unwrap_or(0.0);

    // Filter logic
    let all_values: Vec<f64> = data.months.iter().map(|m| parse_decimal(&m.total).unwrap_or(0.0)).collect();
    let max_val = all_values.iter().fold(0.0f64, |a, &b| a.max(b));

    let filtered_months: Vec<_> = data.months.iter().filter(|m| {
        let val = parse_decimal(&m.total).unwrap_or(0.0);
        
        // Rule 1: If All Time (999), filter zeros.
        if months_filter == 999 && val <= 0.001 {
            return false;
        }
        
        // Rule 2: If "Significant Only", filter low values (< 10% of max).
        if show_significant {
            if val < (max_val * 0.1) {
                return false;
            }
        }
        
        true
    }).cloned().collect();

    let categories: Vec<String> = filtered_months
        .iter()
        .map(|m| format_month_label(&m.month))
        .collect();
    let values: Vec<f64> = filtered_months
        .iter()
        .map(|m| {
            let val = parse_decimal(&m.total).unwrap_or(0.0);
            (val * 100.0).round() / 100.0
        })
        .collect();
    let peak_value = values.iter().copied().fold(0.0, f64::max);
    let trough_value = values
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    let rolling = moving_average(&values, 3);

    let mut series = Vec::with_capacity(2);
    series.push(ChartSeriesData {
        name: "Gasto mensual".to_string(),
        data: values.clone(),
    });
    series.push(ChartSeriesData {
        name: "Media móvil 3M".to_string(),
        data: rolling,
    });

    view! {
        <div class="space-y-6">
            {/* KPIs */}
            <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 animate-fade-in">
                <KpiCard
                    title="Mes actual".to_string()
                    value=current_total
                    decimals=2
                    prefix="€".to_string()
                    trend=Some(data.month_over_month)
                    icon="💫".to_string()
                />
                <KpiCard
                    title="Mes anterior".to_string()
                    value=previous_total
                    decimals=2
                    prefix="€".to_string()
                    trend=None
                    icon="↩︎".to_string()
                    class="lg:col-span-1".to_string()
                />
                <KpiCard
                    title="Media mensual".to_string()
                    value=average_monthly
                    decimals=2
                    prefix="€".to_string()
                    trend=None
                    icon="〰️".to_string()
                />
                <KpiCard
                    title="Total YTD".to_string()
                    value=year_to_date
                    decimals=2
                    prefix="€".to_string()
                    trend=None
                    icon="🔥".to_string()
                />
            </div>

            {/* Chart */}
            <div class="animate-fade-in delay-150">
                <div class="bg-white rounded-2xl border border-gray-100 shadow-sm p-4 sm:p-6">
                    <div class="flex items-center justify-between mb-4">
                        <div>
                            <p class="text-xs uppercase tracking-wider text-gray-500 font-semibold">
                                "Serie histórica"
                            </p>
                            <h2 class="text-xl font-bold text-gray-900">"Evolución de gasto mensual"</h2>
                        </div>
                        <span class="px-3 py-1 text-xs font-semibold bg-primary-50 text-primary-700 rounded-full">
                            {format!("{} puntos", categories.len())}
                        </span>
                    </div>

                    <Chart
                        id="monthly-evolution-chart".to_string()
                        chart_type=ChartType::Area
                        series=series
                        categories=categories.clone()
                        height=420
                        title="".to_string()
                        class="shadow-none".to_string()
                    />
                </div>
            </div>

            {/* Table */}
            <div class="bg-white rounded-2xl border border-gray-100 shadow-sm p-4 sm:p-6 animate-fade-in delay-200">
                <div class="flex items-center justify-between mb-4">
                    <div>
                        <p class="text-xs uppercase tracking-wider text-gray-500 font-semibold">
                            "Detalle mes a mes"
                        </p>
                        <h3 class="text-lg font-bold text-gray-900">"Variación y tickets"</h3>
                    </div>
                </div>

                <div class="overflow-x-auto">
                    <table class="w-full text-left">
                        <thead>
                            <tr class="text-xs text-gray-500 uppercase tracking-wider border-b border-gray-100">
                                <th class="pb-3 pr-4">"Mes"</th>
                                <th class="pb-3 pr-4">"Gasto"</th>
                                <th class="pb-3 pr-4">"Variación vs. anterior"</th>
                                <th class="pb-3 pr-4">"Tickets"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-gray-100">
                            {filtered_months.iter().enumerate().map(|(idx, point)| {
                                let value = parse_decimal(&point.total).unwrap_or(0.0);
                                let previous = if idx > 0 {
                                    parse_decimal(&filtered_months[idx - 1].total).unwrap_or(0.0)
                                } else {
                                    0.0
                                };

                                let delta = if previous.abs() > f64::EPSILON {
                                    ((value - previous) / previous) * 100.0
                                } else if value > 0.0 {
                                    100.0
                                } else {
                                    0.0
                                };

                                let is_peak = value >= peak_value;
                                let is_trough = trough_value.is_finite() && value <= trough_value;

                                view! {
                                    <tr class="hover:bg-gray-50 transition-colors">
                                        <td class="py-3 pr-4 text-sm font-semibold text-gray-900">
                                            {format_month_label(&point.month)}
                                        </td>
                                        <td class="py-3 pr-4 text-sm text-gray-800 font-medium">
                                            {format!("€{:.2}", value)}
                                        </td>
                                        <td class="py-3 pr-4 text-sm">
                                            {
                                                let class = if delta >= 0.0 {
                                                    "text-emerald-600 font-semibold"
                                                } else {
                                                    "text-red-600 font-semibold"
                                                };
                                                view! {
                                                    <span class=class>
                                                        {format!("{:+.1}%", delta)}
                                                    </span>
                                                }
                                            }
                                        </td>
                                        <td class="py-3 pr-4 text-sm text-gray-700 flex items-center gap-2">
                                            {point.ticket_count}
                                            {if is_peak {
                                                view! { <span class="px-2 py-1 text-[11px] font-semibold rounded-full bg-primary-50 text-primary-700">"pico"</span> }.into_view()
                                            } else if is_trough {
                                                view! { <span class="px-2 py-1 text-[11px] font-semibold rounded-full bg-gray-100 text-gray-600">"mínimo"</span> }.into_view()
                                            } else {
                                                view! { <></> }.into_view()
                                            }}
                                        </td>
                                    </tr>
                                }
                            }).collect_view()}
                        </tbody>
                    </table>
                </div>
            </div>
        </div>
    }
}

#[component]
fn MonthlySkeleton() -> impl IntoView {
    view! {
        <div class="space-y-4">
            <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
                {(0..4).map(|_| view! {
                    <div class="h-28 rounded-2xl bg-white/60 border border-gray-100 animate-pulse" />
                }).collect_view()}
            </div>
            <div class="h-[420px] rounded-2xl bg-white/60 border border-gray-100 animate-pulse" />
            <div class="h-64 rounded-2xl bg-white/60 border border-gray-100 animate-pulse" />
        </div>
    }
}

fn parse_decimal(input: &str) -> Option<f64> {
    input.replace(',', ".").parse::<f64>().ok()
}

fn format_month_label(raw: &str) -> String {
    let parts: Vec<&str> = raw.split('-').collect();
    if parts.len() == 2 {
        let month = match parts[1] {
            "01" => "Ene",
            "02" => "Feb",
            "03" => "Mar",
            "04" => "Abr",
            "05" => "May",
            "06" => "Jun",
            "07" => "Jul",
            "08" => "Ago",
            "09" => "Sep",
            "10" => "Oct",
            "11" => "Nov",
            "12" => "Dic",
            _ => parts[1],
        };
        format!("{} {}", month, parts[0])
    } else {
        raw.to_string()
    }
}

fn moving_average(values: &[f64], window: usize) -> Vec<f64> {
    if window == 0 || values.is_empty() {
        return Vec::new();
    }

    values
        .iter()
        .enumerate()
        .map(|(idx, _)| {
            let start = idx.saturating_sub(window - 1);
            let slice = &values[start..=idx];
            let sum: f64 = slice.iter().sum();
            sum / slice.len() as f64
        })
        .collect()
}
