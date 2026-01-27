use crate::api::prediction::PredictionResult;
use chrono::{DateTime, FixedOffset};
use leptos::*;

#[component]
pub fn PredictionCard(#[prop(into)] prediction: PredictionResult) -> impl IntoView {
    let has_products = !prediction.suggested_products.is_empty();
    let formatted_date = prediction
        .timestamp
        .parse::<DateTime<FixedOffset>>()
        .ok()
        .map(|dt| dt.format("%d/%m/%Y").to_string())
        .unwrap_or_else(|| prediction.timestamp.clone());

    view! {
        <div class="w-full max-w-3xl mx-auto p-6 bg-white rounded-2xl shadow-sm border border-gray-100 animate-fade-in space-y-6">
            <header class="flex flex-col gap-2">
                <p class="text-xs uppercase tracking-[0.18rem] text-primary-500 font-semibold">"Predicción de compra"</p>
                <h2 class="text-2xl font-bold text-gray-900 leading-tight">
                    "Tu próxima visita será " <span class="text-primary-700">{prediction.day_label.clone()}</span>
                    {format!(", entre {}", prediction.time_window_range)}
                </h2>
                <p class="text-xs text-gray-500">
                    {"Fecha exacta: "} {formatted_date.clone()}
                </p>
            </header>

            <div class="space-y-6">
                <div>
                    <h3 class="text-sm font-semibold text-gray-900 uppercase tracking-wider mb-3">"Productos sugeridos (cesta base)"</h3>
                    {move || if has_products {
                        view! {
                            <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                                {prediction.suggested_products.clone().into_iter().map(|product| {
                                    view! {
                                        <div class="p-4 rounded-xl border border-gray-100 hover:border-primary-200 hover:shadow-md transition-all bg-white group">
                                            <div class="flex justify-between items-start mb-2">
                                                <span class="font-medium text-gray-900 group-hover:text-primary-600 transition-colors">
                                                    {product.name}
                                                </span>
                                                <span class="text-[11px] font-semibold px-2 py-1 rounded-full bg-emerald-50 text-emerald-700">
                                                    {format!("{:.0}%", product.probability * 100.0)}
                                                </span>
                                            </div>
                                            <p class="text-xs text-gray-500 mb-1">{product.reason}</p>
                                            {move || if product.price_estimation > 0.0 {
                                                view! {
                                                    <div class="inline-flex items-center gap-1 px-2 py-1 rounded-lg bg-primary-50 text-primary-700 text-xs font-semibold">
                                                        <span>{"Precio:"}</span>
                                                        <span>{format!("{:.2}€", product.price_estimation)}</span>
                                                    </div>
                                                }.into_view()
                                            } else {
                                                view! { <></> }.into_view()
                                            }}
                                        </div>
                                    }
                                }).collect::<Vec<_>>()}
                            </div>
                        }.into_view()
                    } else {
                        view! {
                            <div class="p-4 rounded-xl border border-dashed border-gray-200 bg-gray-50 text-sm text-gray-600">
                                "Aún no tengo suficientes tickets para listar tu cesta base, pero seguiré aprendiendo."
                            </div>
                        }.into_view()
                    }}
                </div>

                <div class="pt-4 border-t border-gray-100 flex items-center flex-wrap gap-3 justify-between text-xs text-gray-500">
                    <span class="font-medium">{format!("Confianza del modelo: {:.0}%", prediction.confidence * 100.0)}</span>
                    <span>{format!("Actualizado: {}", prediction.timestamp)}</span>
                </div>
            </div>
        </div>
    }
}
