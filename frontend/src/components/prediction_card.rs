use leptos::*;
use crate::api::prediction::PredictionResult;

#[component]
pub fn PredictionCard(
    #[prop(into)] prediction: PredictionResult
) -> impl IntoView {
    view! {
        <div class="w-full max-w-2xl mx-auto p-6 bg-white rounded-xl shadow-sm border border-gray-100 animate-fade-in">
            <div class="flex items-start justify-between mb-6">
                <div>
                    <h2 class="text-2xl font-bold text-gray-900 mb-1">"Próxima Compra"</h2>
                    <p class="text-gray-500 text-sm">
                        {format!("Estimada para {}", prediction.time_window_label)}
                    </p>
                </div>
                <div class="bg-primary-50 text-primary-700 px-4 py-2 rounded-lg font-semibold">
                    {format!("~{:.2}€", prediction.estimated_total)}
                </div>
            </div>

            <div class="space-y-6">
                <div>
                    <h3 class="text-sm font-semibold text-gray-900 uppercase tracking-wider mb-3">
                        "Productos Sugeridos"
                    </h3>
                    <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                        {prediction.suggested_products.into_iter().map(|product| {
                            view! {
                                <div class="p-4 rounded-lg border border-gray-100 hover:border-primary-200 hover:shadow-md transition-all bg-white group">
                                    <div class="flex justify-between items-start mb-2">
                                        <span class="font-medium text-gray-900 group-hover:text-primary-600 transition-colors">
                                            {product.name}
                                        </span>
                                        <span class="text-xs font-medium px-2 py-1 rounded-full bg-green-50 text-green-700">
                                            {format!("{:.0}%", product.probability * 100.0)}
                                        </span>
                                    </div>
                                    <p class="text-xs text-gray-500 mb-2">{product.reason}</p>
                                    <div class="text-sm font-semibold text-gray-900">
                                        {format!("~{:.2}€", product.price_estimation)}
                                    </div>
                                </div>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                </div>

                <div class="pt-4 border-t border-gray-100 flex items-center justify-between text-xs text-gray-400">
                    <span>{format!("Confianza del modelo: {:.0}%", prediction.confidence * 100.0)}</span>
                    <span>{format!("Actualizado: {}", prediction.timestamp)}</span>
                </div>
            </div>
        </div>
    }
}
