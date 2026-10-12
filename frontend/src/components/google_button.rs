use crate::api::google::get_providers;
use leptos::*;

/// Botón "Continuar con Google" (con el separador "o"). No se muestra si el servidor no tiene
/// configurado el inicio de sesión con Google.
///
/// Es un enlace normal y no una llamada `fetch`: el flujo OAuth necesita una navegación completa
/// del navegador a Google. `rel="external"` evita que el router de Leptos intercepte el clic.
#[component]
pub fn GoogleButton(
    /// Texto del botón
    #[prop(into)]
    label: String,
) -> impl IntoView {
    let available = create_resource(
        || (),
        |_| async { get_providers().await.map(|p| p.google).unwrap_or(false) },
    );

    view! {
        {move || available.get().unwrap_or(false).then(|| {
            let label = label.clone();
            view! {
                <div class="mt-6">
                    <div class="relative flex items-center">
                        <div class="flex-grow border-t border-gray-200"></div>
                        <span class="mx-4 text-xs uppercase tracking-wide text-gray-400">"o"</span>
                        <div class="flex-grow border-t border-gray-200"></div>
                    </div>
                    <a
                        href="/api/auth/google/start"
                        rel="external"
                        class="mt-6 w-full inline-flex items-center justify-center gap-3 px-6 py-3 rounded-lg border border-gray-300 bg-white text-gray-700 font-medium shadow-sm hover:bg-gray-50 hover:shadow-md transition-all focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-primary-500"
                    >
                        <GoogleLogo />
                        {label}
                    </a>
                    <p class="mt-3 text-xs text-center text-gray-500">
                        "Con Google también podemos importar automáticamente los tickets que Mercadona te envía por correo (solo lectura)."
                    </p>
                </div>
            }
        })}
    }
}

/// Logotipo "G" de Google (colores oficiales).
#[component]
pub fn GoogleLogo() -> impl IntoView {
    view! {
        <svg class="w-5 h-5" viewBox="0 0 48 48" aria-hidden="true">
            <path fill="#EA4335" d="M24 9.5c3.54 0 6.71 1.22 9.21 3.6l6.85-6.85C35.9 2.38 30.47 0 24 0 14.62 0 6.51 5.38 2.56 13.22l7.98 6.19C12.43 13.72 17.74 9.5 24 9.5z"/>
            <path fill="#4285F4" d="M46.98 24.55c0-1.57-.15-3.09-.38-4.55H24v9.02h12.94c-.58 2.96-2.26 5.48-4.78 7.18l7.73 6c4.51-4.18 7.09-10.36 7.09-17.65z"/>
            <path fill="#FBBC05" d="M10.53 28.59c-.48-1.45-.76-2.99-.76-4.59s.27-3.14.76-4.59l-7.98-6.19C.92 16.46 0 20.12 0 24c0 3.88.92 7.54 2.56 10.78l7.97-6.19z"/>
            <path fill="#34A853" d="M24 48c6.48 0 11.93-2.13 15.89-5.81l-7.73-6c-2.15 1.45-4.92 2.3-8.16 2.3-6.26 0-11.57-4.22-13.47-9.91l-7.98 6.19C6.51 42.62 14.62 48 24 48z"/>
        </svg>
    }
}
