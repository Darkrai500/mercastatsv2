use leptos::*;
use gloo_timers::future::TimeoutFuture;
use crate::api::prediction::{get_next_prediction, PredictionResult};
use crate::components::ai_loader::AIGeneratingLoader;
use crate::components::prediction_card::PredictionCard;

#[component]
pub fn Prediction() -> impl IntoView {
    let (prediction, set_prediction) = create_signal::<Option<PredictionResult>>(None);
    let (loading, set_loading) = create_signal(true);
    let (error, set_error) = create_signal::<Option<String>>(None);

    create_effect(move |_| {
        spawn_local(async move {
            // Artificial delay for "AI thinking" effect
            TimeoutFuture::new(1500).await;
            
            match get_next_prediction().await {
                Ok(res) => {
                    set_prediction.set(Some(res.prediction));
                    set_loading.set(false);
                },
                Err(e) => {
                    set_error.set(Some(e));
                    set_loading.set(false);
                }
            }
        });
    });

    view! {
        <div class="space-y-6">
            <div class="flex items-center justify-between">
                <h1 class="text-2xl font-bold text-gray-900">"Predicción de Compra"</h1>
            </div>

            {move || {
                if loading.get() {
                    view! { <AIGeneratingLoader /> }.into_view()
                } else if let Some(err) = error.get() {
                    view! {
                        <div class="p-4 bg-red-50 text-red-700 rounded-lg border border-red-100">
                            {err}
                        </div>
                    }.into_view()
                    } else if let Some(pred) = prediction.get() {
                        if pred.learning_mode {
                            view! {
                                <div class="text-center p-8 bg-blue-50 rounded-xl border border-blue-100 animate-fade-in">
                                    <div class="mb-4 flex justify-center">
                                    <svg class="w-12 h-12 text-blue-500 animate-pulse" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19.428 15.428a2 2 0 00-1.022-.547l-2.384-.477a6 6 0 00-3.86.517l-.318.158a6 6 0 01-3.86.517L6.05 15.21a2 2 0 00-1.806.547M8 4h8l-1 1v5.172a2 2 0 00.586 1.414l5 5c1.26 1.26.367 3.414-1.415 3.414H4.828c-1.782 0-2.674-2.154-1.414-3.414l5-5A2 2 0 009 10.172V5L8 4z"></path>
                                    </svg>
                                </div>
                                <h3 class="text-lg font-bold text-blue-900 mb-2">"Afinando mis neuronas..."</h3>
                                <p class="text-blue-700">"Todavía estoy aprendiendo tus patrones de consumo. Necesito analizar unas cuantas compras más para ser preciso."</p>
                            </div>
                        }.into_view()
                    } else {
                        view! { <PredictionCard prediction=pred /> }.into_view()
                    }
                } else {
                    view! { <div>"No hay datos disponibles"</div> }.into_view()
                }
            }}
        </div>
    }
}
