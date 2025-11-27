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
                    view! { <PredictionCard prediction=pred /> }.into_view()
                } else {
                    view! { <div>"No hay datos disponibles"</div> }.into_view()
                }
            }}
        </div>
    }
}
