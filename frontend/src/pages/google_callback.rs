use crate::api::google::{error_message, set_flash};
use leptos::*;
use leptos_router::use_navigate;

/// Destino al que el backend redirige tras el flujo de Google. La información viaja en el
/// fragmento de la URL (`#token=…`), que el navegador no envía a ningún servidor; se guarda y se
/// borra del historial de inmediato.
///
/// - `#token=…&email=…&is_demo=…`  inicio de sesión correcto
/// - `#linked=1&mode=link`          Gmail conectado desde una sesión ya iniciada
/// - `#error=…&mode=login|link`     algo ha fallado
#[component]
pub fn GoogleCallback() -> impl IntoView {
    let navigate = use_navigate();

    create_effect(move |_| {
        let Some(window) = web_sys::window() else { return };
        let hash = window.location().hash().unwrap_or_default();
        let params = web_sys::UrlSearchParams::new_with_str(hash.trim_start_matches('#'))
            .ok();
        let get = |key: &str| params.as_ref().and_then(|p| p.get(key));

        // Quita el fragmento (con el JWT) de la barra de direcciones y del historial
        if let Ok(history) = window.history() {
            let _ = history.replace_state_with_url(
                &wasm_bindgen::JsValue::NULL,
                "",
                Some("/auth/google"),
            );
        }

        let linking = get("mode").as_deref() == Some("link");

        if let Some(code) = get("error") {
            set_flash(&error_message(&code), true);
            navigate(if linking { "/dashboard" } else { "/" }, Default::default());
            return;
        }

        if get("linked").is_some() {
            set_flash("Gmail conectado. Estamos importando tus tickets…", false);
            navigate("/dashboard", Default::default());
            return;
        }

        match (get("token"), get("email")) {
            (Some(token), Some(email)) if !token.is_empty() => {
                if let Ok(Some(storage)) = window.local_storage() {
                    let _ = storage.set_item("auth_token", &token);
                    let _ = storage.set_item("user_email", &email);
                    let _ = storage.set_item(
                        "user_is_demo",
                        &(get("is_demo").as_deref() == Some("true")).to_string(),
                    );
                }
                navigate("/dashboard", Default::default());
            }
            _ => {
                set_flash(&error_message("estado_invalido"), true);
                navigate("/", Default::default());
            }
        }
    });

    view! {
        <div class="min-h-screen bg-gradient-to-br from-gray-50 via-white to-primary-50 flex items-center justify-center p-4">
            <div class="text-center">
                <svg class="mx-auto h-8 w-8 animate-spin text-primary-600" fill="none" viewBox="0 0 24 24">
                    <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                    <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                </svg>
                <p class="mt-4 text-gray-600">"Iniciando sesión…"</p>
            </div>
        </div>
    }
}
