use leptos::*;
use web_sys::{File, HtmlInputElement};
use crate::components::{Button, ButtonVariant, Card};
use crate::api::tickets::upload_ticket;

#[component]
pub fn Upload() -> impl IntoView {
    let (selected_file, set_selected_file) = create_signal(None::<File>);
    let (preview_url, set_preview_url) = create_signal(None::<String>);
    let (uploading, set_uploading) = create_signal(false);
    let (success_message, set_success_message) = create_signal(None::<String>);
    let (error_message, set_error_message) = create_signal(None::<String>);
    let (_user_email, set_user_email) = create_signal(None::<String>);

    // Obtener email del usuario de localStorage
    create_effect(move |_| {
        if let Some(window) = web_sys::window() {
            if let Ok(Some(storage)) = window.local_storage() {
                if let Ok(Some(email)) = storage.get_item("user_email") {
                    set_user_email.set(Some(email));
                }
            }
        }
    });

    let file_input_ref = create_node_ref::<html::Input>();

    let handle_file_select = move |ev: web_sys::Event| {
        let target = event_target::<HtmlInputElement>(&ev);
        if let Some(files) = target.files() {
            if let Some(file) = files.get(0) {
                set_selected_file.set(Some(file.clone()));
                set_error_message.set(None);
                set_success_message.set(None);

                // Crear preview para imágenes
                let file_type = file.type_();
                if file_type.starts_with("image/") {
                    spawn_local(async move {
                        if let Ok(url) = create_object_url(&file) {
                            set_preview_url.set(Some(url));
                        }
                    });
                } else {
                    set_preview_url.set(None);
                }
            }
        }
    };

    let _handle_upload_click = move |_: leptos::ev::MouseEvent| {
        if let Some(file) = selected_file.get() {
            set_uploading.set(true);
            set_error_message.set(None);
            set_success_message.set(None);

            spawn_local(async move {
                match upload_ticket(file).await {
                    Ok(response) => {
                        set_success_message.set(Some(format!(
                            "¡Ticket subido con éxito! ID: {}",
                            response.ticket_id
                        )));
                        set_uploading.set(false);
                        set_selected_file.set(None);
                        set_preview_url.set(None);

                        // Reset file input
                        if let Some(input) = file_input_ref.get() {
                            input.set_value("");
                        }
                    }
                    Err(err) => {
                        set_error_message.set(Some(err));
                        set_uploading.set(false);
                    }
                }
            });
        }
    };


    let trigger_file_input = move |_| {
        if let Some(input) = file_input_ref.get() {
            input.click();
        }
    };

    view! {
        <div class="space-y-6">
            // Header
            <div>
                <h1 class="text-3xl font-bold text-gray-900 mb-2">
                    "Sube tu ticket de Mercadona"
                </h1>
                <p class="text-gray-600">
                    "Analiza tus compras y descubre patrones de consumo"
                </p>
            </div>

            // Messages
            {move || success_message.get().map(|msg| view! {
                <div class="p-4 bg-green-50 border border-green-200 rounded-xl animate-slide-up">
                    <div class="flex items-center">
                        <svg class="w-6 h-6 text-green-600 mr-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"></path>
                        </svg>
                        <p class="text-green-800 font-medium">{msg}</p>
                    </div>
                </div>
            })}

            {move || error_message.get().map(|msg| view! {
                <div class="p-4 bg-red-50 border border-red-200 rounded-xl animate-slide-up">
                    <div class="flex items-center">
                        <svg class="w-6 h-6 text-red-600 mr-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path>
                        </svg>
                        <p class="text-red-800 font-medium">{msg}</p>
                    </div>
                </div>
            })}

            // Upload card
            <Card class="animate-slide-up".to_string()>
                <div class="space-y-6">
                    // Drop zone
                    <div
                            class="relative border-3 border-dashed border-gray-300 rounded-xl p-12 text-center hover:border-primary-400 transition-colors cursor-pointer group"
                            on:click=trigger_file_input
                        >
                            <input
                                node_ref=file_input_ref
                                type="file"
                                accept="image/*,.pdf"
                                class="hidden"
                                on:change=handle_file_select
                            />

                            {move || if let Some(_) = selected_file.get() {
                                view! {
                                    <div class="space-y-4">
                                        {move || preview_url.get().map(|url| view! {
                                            <img
                                                src={url}
                                                alt="Preview"
                                                class="max-h-64 mx-auto rounded-lg shadow-md"
                                            />
                                        })}
                                        <div class="flex items-center justify-center text-green-600">
                                            <svg class="w-12 h-12" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"></path>
                                            </svg>
                                        </div>
                                        <div>
                                            <p class="text-lg font-semibold text-gray-900">
                                                {move || selected_file.get().map(|f| f.name()).unwrap_or_default()}
                                            </p>
                                            <p class="text-sm text-gray-500">
                                                {move || {
                                                    selected_file.get().map(|f| {
                                                        let size = f.size() as f64;
                                                        if size < 1024.0 {
                                                            format!("{:.0} B", size)
                                                        } else if size < 1024.0 * 1024.0 {
                                                            format!("{:.1} KB", size / 1024.0)
                                                        } else {
                                                            format!("{:.1} MB", size / (1024.0 * 1024.0))
                                                        }
                                                    }).unwrap_or_default()
                                                }}
                                            </p>
                                        </div>
                                        <p class="text-sm text-gray-600">
                                            "Haz clic para cambiar el archivo"
                                        </p>
                                    </div>
                                }.into_view()
                            } else {
                                view! {
                                    <div class="space-y-4">
                                        <div class="flex justify-center">
                                            <div class="w-24 h-24 bg-primary-100 rounded-full flex items-center justify-center group-hover:bg-primary-200 transition-colors">
                                                <svg class="w-12 h-12 text-primary-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12"></path>
                                                </svg>
                                            </div>
                                        </div>
                                        <div>
                                            <p class="text-lg font-semibold text-gray-900 mb-2">
                                                "Arrastra tu ticket aquí o haz clic para seleccionar"
                                            </p>
                                            <p class="text-sm text-gray-600">
                                                "Soportamos PDF e imágenes (JPG, PNG)"
                                            </p>
                                            <p class="text-xs text-gray-500 mt-2">
                                                "Tamaño máximo: 10MB"
                                            </p>
                                        </div>
                                    </div>
                                }.into_view()
                            }}
                        </div>

                        // Action buttons
                        <div class="flex gap-4">
                            {move || if selected_file.get().is_some() {
                                view! {
                                    <>
                                        <Button
                                            full_width=true
                                            loading=uploading.get()
                                            disabled=uploading.get()
                                        >
                                            <svg class="w-5 h-5 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12"></path>
                                            </svg>
                                            {move || if uploading.get() { "Subiendo..." } else { "Subir ticket" }}
                                        </Button>
                                        <Button
                                            variant=ButtonVariant::Outline
                                        >
                                            "Cancelar"
                                        </Button>
                                    </>
                                }.into_view()
                            } else {
                                view! { <div></div> }.into_view()
                            }}
                        </div>

                        // Info cards
                        <div class="grid grid-cols-1 md:grid-cols-3 gap-4 pt-6 border-t border-gray-200">
                            <div class="text-center p-4 bg-primary-50 rounded-lg">
                                <div class="text-3xl font-bold text-primary-600 mb-2">"0"</div>
                                <div class="text-sm text-gray-600">"Tickets subidos"</div>
                            </div>
                            <div class="text-center p-4 bg-accent-50 rounded-lg">
                                <div class="text-3xl font-bold text-accent-600 mb-2">"0€"</div>
                                <div class="text-sm text-gray-600">"Gasto total"</div>
                            </div>
                            <div class="text-center p-4 bg-green-50 rounded-lg">
                                <div class="text-3xl font-bold text-green-600 mb-2">"0"</div>
                                <div class="text-sm text-gray-600">"Productos únicos"</div>
                            </div>
                        </div>
                    </div>
            </Card>

            // Tips section
            <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
                <Card padding=true>
                    <div class="flex items-start space-x-4">
                        <div class="flex-shrink-0">
                            <div class="w-12 h-12 bg-blue-100 rounded-lg flex items-center justify-center">
                                <svg class="w-6 h-6 text-blue-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path>
                                </svg>
                            </div>
                        </div>
                        <div>
                            <h3 class="font-semibold text-gray-900 mb-2">"Consejos para mejores resultados"</h3>
                            <ul class="text-sm text-gray-600 space-y-1">
                                <li>"• Asegúrate de que el ticket esté bien iluminado"</li>
                                <li>"• Evita sombras y reflejos"</li>
                                <li>"• Captura el ticket completo"</li>
                            </ul>
                        </div>
                    </div>
                </Card>

                <Card padding=true>
                    <div class="flex items-start space-x-4">
                        <div class="flex-shrink-0">
                            <div class="w-12 h-12 bg-purple-100 rounded-lg flex items-center justify-center">
                                <svg class="w-6 h-6 text-purple-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z"></path>
                                </svg>
                            </div>
                        </div>
                        <div>
                            <h3 class="font-semibold text-gray-900 mb-2">"Tu privacidad es importante"</h3>
                            <p class="text-sm text-gray-600">
                                "Tus tickets se almacenan de forma segura y solo tú puedes acceder a ellos."
                            </p>
                        </div>
                    </div>
                </Card>
            </div>
        </div>
    }
}

fn create_object_url(file: &File) -> Result<String, String> {
    web_sys::Url::create_object_url_with_blob(file)
        .map_err(|_| "Error al crear preview".to_string())
}
