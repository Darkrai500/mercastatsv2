use leptos::*;
use leptos_router::*;
use crate::components::{Button, ButtonVariant, Card};
use crate::api::auth::{login_user, LoginRequest};

#[component]
pub fn Login() -> impl IntoView {
    let (email, set_email) = create_signal(String::new());
    let (password, set_password) = create_signal(String::new());
    let (error, set_error) = create_signal(None::<String>);
    let (loading, set_loading) = create_signal(false);

    let navigate = use_navigate();

    let navigate_clone = navigate.clone();
    let handle_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();

        set_error.set(None);
        set_loading.set(true);

        let email_val = email.get();
        let password_val = password.get();

        // Validación básica
        if email_val.is_empty() || password_val.is_empty() {
            set_error.set(Some("Por favor, completa todos los campos".to_string()));
            set_loading.set(false);
            return;
        }

        if !email_val.contains('@') {
            set_error.set(Some("Por favor, introduce un email válido".to_string()));
            set_loading.set(false);
            return;
        }

        let navigate_for_spawn = navigate_clone.clone();
        spawn_local(async move {
            let request = LoginRequest {
                email: email_val.clone(),
                password: password_val.clone(),
            };

            match login_user(request).await {
                Ok(response) => {
                    // Guardar token en localStorage
                    if let Some(window) = web_sys::window() {
                        if let Ok(Some(storage)) = window.local_storage() {
                            let _ = storage.set_item("auth_token", &response.token);
                            let _ = storage.set_item("user_email", &email_val);
                        }
                    }

                    set_loading.set(false);
                    // Navegar a la página de upload
                    navigate_for_spawn("/upload", Default::default());
                }
                Err(err) => {
                    set_error.set(Some(err));
                    set_loading.set(false);
                }
            }
        });
    };

    view! {
        <div class="min-h-screen bg-gradient-to-br from-gray-50 via-white to-primary-50 flex items-center justify-center p-4 animate-fade-in">
            <div class="w-full max-w-md animate-slide-up">
                // Logo y título
                <div class="text-center mb-8">
                    <div class="inline-flex items-center justify-center w-16 h-16 bg-primary-600 rounded-2xl mb-4 shadow-lg">
                        <svg class="w-8 h-8 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 7h6m0 10v-3m-3 3h.01M9 17h.01M9 14h.01M12 14h.01M15 11h.01M12 11h.01M9 11h.01M7 21h10a2 2 0 002-2V5a2 2 0 00-2-2H7a2 2 0 00-2 2v14a2 2 0 002 2z"></path>
                        </svg>
                    </div>
                    <h1 class="text-3xl font-bold text-gray-900 mb-2">
                        "Mercastats"
                    </h1>
                    <p class="text-gray-600">
                        "Analiza tus compras de Mercadona"
                    </p>
                </div>

                // Card de login
                <Card>
                    <form on:submit=handle_submit class="space-y-6">
                        <div class="text-center mb-6">
                            <h2 class="text-xl font-semibold text-gray-900">
                                "Iniciar sesión"
                            </h2>
                        </div>

                        {move || error.get().map(|err| view! {
                            <div class="p-4 bg-red-50 border border-red-200 rounded-lg animate-fade-in">
                                <p class="text-sm text-red-800 text-center">{err}</p>
                            </div>
                        })}

                        <div class="space-y-1">
                            <label class="block text-sm font-medium text-gray-700 mb-2">
                                "Email"
                                <span class="text-red-500 ml-1">"*"</span>
                            </label>
                            <input
                                type="email"
                                class="w-full px-4 py-3 rounded-lg border-2 border-gray-200 focus:border-primary-500 focus:ring-2 focus:ring-primary-200 outline-none transition-all bg-white text-gray-900 placeholder-gray-400"
                                placeholder="tu@email.com"
                                value={move || email.get()}
                                on:input=move |ev| set_email.set(event_target_value(&ev))
                                autocomplete="email"
                            />
                        </div>

                        <div class="space-y-1">
                            <label class="block text-sm font-medium text-gray-700 mb-2">
                                "Contraseña"
                                <span class="text-red-500 ml-1">"*"</span>
                            </label>
                            <input
                                type="password"
                                class="w-full px-4 py-3 rounded-lg border-2 border-gray-200 focus:border-primary-500 focus:ring-2 focus:ring-primary-200 outline-none transition-all bg-white text-gray-900 placeholder-gray-400"
                                placeholder="••••••••"
                                value={move || password.get()}
                                on:input=move |ev| set_password.set(event_target_value(&ev))
                                autocomplete="current-password"
                            />
                        </div>

                        <div class="flex items-center justify-between text-sm">
                            <label class="flex items-center">
                                <input type="checkbox" class="w-4 h-4 rounded border-gray-300 text-primary-600 focus:ring-primary-500" />
                                <span class="ml-2 text-gray-600">"Recordarme"</span>
                            </label>
                            <a href="#" class="text-primary-600 hover:text-primary-700 font-medium">
                                "¿Olvidaste tu contraseña?"
                            </a>
                        </div>

                        <Button
                            button_type="submit".to_string()
                            full_width=true
                            loading=loading.get()
                            disabled=loading.get()
                        >
                            {move || if loading.get() { "Iniciando sesión..." } else { "Iniciar sesión" }}
                        </Button>

                        <div class="relative my-6">
                            <div class="absolute inset-0 flex items-center">
                                <div class="w-full border-t border-gray-200"></div>
                            </div>
                            <div class="relative flex justify-center text-sm">
                                <span class="px-4 bg-white text-gray-500">"o continúa con"</span>
                            </div>
                        </div>

                        <Button
                            variant=ButtonVariant::Outline
                            full_width=true
                        >
                            <svg class="w-5 h-5 mr-2" viewBox="0 0 24 24">
                                <path fill="#4285F4" d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"/>
                                <path fill="#34A853" d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"/>
                                <path fill="#FBBC05" d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"/>
                                <path fill="#EA4335" d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"/>
                            </svg>
                            "Google"
                        </Button>

                        <p class="text-center text-sm text-gray-600 mt-6">
                            "¿No tienes cuenta? "
                            <a href="#" class="text-primary-600 hover:text-primary-700 font-medium">
                                "Regístrate gratis"
                            </a>
                        </p>
                    </form>
                </Card>

                // Footer
                <p class="text-center text-sm text-gray-500 mt-8">
                    "Al iniciar sesión, aceptas nuestros "
                    <a href="#" class="text-primary-600 hover:text-primary-700">"Términos"</a>
                    " y "
                    <a href="#" class="text-primary-600 hover:text-primary-700">"Política de Privacidad"</a>
                </p>
            </div>
        </div>
    }
}
