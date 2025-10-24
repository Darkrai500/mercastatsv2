mod components;
mod pages;
mod api;

use leptos::*;
use leptos_router::*;
use leptos_meta::*;
use wasm_bindgen::prelude::*;
use pages::{Login, Upload};

#[wasm_bindgen(start)]
pub fn main() {
    // Setup console logging
    console_log::init_with_level(log::Level::Debug).expect("error initializing logger");
    console_error_panic_hook::set_once();

    log::info!("Mercastats frontend iniciado");

    mount_to_body(|| {
        view! {
            <App />
        }
    })
}

#[component]
fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Router>
            <Meta name="charset" content="utf-8" />
            <Meta name="viewport" content="width=device-width, initial-scale=1.0" />
            <Title text="Mercastats - Analiza tus compras de Mercadona" />

            <Routes>
                <Route path="/" view=Login />
                <Route path="/upload" view=Upload />
                <Route path="/*any" view=NotFound />
            </Routes>
        </Router>
    }
}

#[component]
fn NotFound() -> impl IntoView {
    view! {
        <div class="min-h-screen bg-gradient-to-br from-gray-50 via-white to-primary-50 flex items-center justify-center p-4">
            <div class="text-center max-w-md animate-fade-in">
                <div class="mb-8">
                    <div class="inline-flex items-center justify-center w-24 h-24 bg-red-100 rounded-full mb-6">
                        <svg class="w-12 h-12 text-red-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"></path>
                        </svg>
                    </div>
                    <h1 class="text-6xl font-bold text-gray-900 mb-4">"404"</h1>
                    <h2 class="text-2xl font-semibold text-gray-800 mb-4">
                        "Página no encontrada"
                    </h2>
                    <p class="text-gray-600 mb-8">
                        "Lo sentimos, la página que buscas no existe."
                    </p>
                    <a
                        href="/"
                        class="inline-flex items-center justify-center px-6 py-3 bg-primary-600 text-white rounded-lg font-medium hover:bg-primary-700 transition-colors shadow-sm hover:shadow-md"
                    >
                        <svg class="w-5 h-5 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 19l-7-7m0 0l7-7m-7 7h18"></path>
                        </svg>
                        "Volver al inicio"
                    </a>
                </div>
            </div>
        </div>
    }
}
