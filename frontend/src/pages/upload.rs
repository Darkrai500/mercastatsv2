use crate::api::tickets::{process_ticket_ocr, ProcessTicketResponse};
use crate::components::{Button, ButtonVariant, Card};
use leptos::*;
use std::collections::HashSet;
use web_sys::{File, HtmlInputElement};

#[derive(Clone, PartialEq)]
enum UploadStatus {
    Pending,
    Processing,
    Success,
    Error,
}

#[derive(Clone, PartialEq)]
struct FileStatus {
    id: String,
    file: File,
    status: UploadStatus,
    result: Option<ProcessTicketResponse>,
    error: Option<String>,
    preview_url: Option<String>,
}

#[component]
pub fn Upload() -> impl IntoView {
    let (files, set_files) = create_signal(Vec::<FileStatus>::new());
    let (processing, set_processing) = create_signal(false);
    let (processing_batch, set_processing_batch) = create_signal(HashSet::<String>::new());
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
        if let Some(file_list) = target.files() {
            let mut new_files = Vec::new();
            for i in 0..file_list.length() {
                if let Some(file) = file_list.get(i) {
                    let id = format!("{}-{}", file.name(), js_sys::Date::now());
                    
                    // Crear preview para imágenes
                    let mut preview_url = None;
                    let file_type = file.type_();
                    if file_type.starts_with("image/") {
                        if let Ok(url) = create_object_url(&file) {
                            preview_url = Some(url);
                        }
                    }

                    new_files.push(FileStatus {
                        id,
                        file,
                        status: UploadStatus::Pending,
                        result: None,
                        error: None,
                        preview_url,
                    });
                }
            }
            
            set_files.update(|current| current.extend(new_files));
            
            // Reset input so same files can be selected again if needed
            if let Some(input) = file_input_ref.get() {
                input.set_value("");
            }
        }
    };

    let process_file = move |file_status: FileStatus| {
        let id = file_status.id.clone();
        
        spawn_local(async move {
            // Update status to processing
            set_files.update(|files| {
                if let Some(f) = files.iter_mut().find(|f| f.id == id) {
                    f.status = UploadStatus::Processing;
                    f.error = None;
                }
            });

            match process_ticket_ocr(file_status.file).await {
                Ok(response) => {
                    set_files.update(|files| {
                        if let Some(f) = files.iter_mut().find(|f| f.id == id) {
                            f.status = UploadStatus::Success;
                            f.result = Some(response);
                        }
                    });
                }
                Err(err) => {
                    let error_msg = if err.contains("ya existe") || err.contains("Conflict") {
                        format!("⚠️ Duplicado: {}", err)
                    } else {
                        err
                    };
                    
                    set_files.update(|files| {
                        if let Some(f) = files.iter_mut().find(|f| f.id == id) {
                            f.status = UploadStatus::Error;
                            f.error = Some(error_msg);
                        }
                    });
                }
            }
            
            // Check if all files in the current batch are done
            let batch_complete = files.with(|files| {
                processing_batch.with(|batch_ids| {
                    batch_ids.iter().all(|batch_id| {
                        files.iter()
                            .find(|f| &f.id == batch_id)
                            .map(|f| f.status == UploadStatus::Success || f.status == UploadStatus::Error)
                            .unwrap_or(true) // If file was removed, consider it done
                    })
                })
            });
            
            if batch_complete {
                set_processing.set(false);
                set_processing_batch.set(HashSet::new());
            }
        });
    };

    let handle_process_all = move |_| {
        let pending_files: Vec<FileStatus> = files.get()
            .into_iter()
            .filter(|f| f.status == UploadStatus::Pending || f.status == UploadStatus::Error)
            .collect();
            
        if pending_files.is_empty() {
            return;
        }

        // Capture IDs of files in this batch
        let batch_ids: HashSet<String> = pending_files.iter()
            .map(|f| f.id.clone())
            .collect();
        
        set_processing_batch.set(batch_ids);
        set_processing.set(true);
        
        for file in pending_files {
            process_file(file);
        }
    };

    let remove_file = move |id: String| {
        set_files.update(|files| {
            if let Some(index) = files.iter().position(|f| f.id == id) {
                let file = &files[index];
                if let Some(url) = &file.preview_url {
                    let _ = web_sys::Url::revoke_object_url(url);
                }
                files.remove(index);
            }
        });
    };

    let clear_completed = move |_| {
        set_files.update(|files| {
            files.retain(|f| {
                if f.status == UploadStatus::Success {
                    if let Some(url) = &f.preview_url {
                        let _ = web_sys::Url::revoke_object_url(url);
                    }
                    false
                } else {
                    true
                }
            });
        });
    };

    let trigger_file_input = move |_| {
        if let Some(input) = file_input_ref.get() {
            input.click();
        }
    };

    on_cleanup(move || {
        // Cleanup all object URLs
        files.with(|files| {
            for file in files {
                if let Some(url) = &file.preview_url {
                    let _ = web_sys::Url::revoke_object_url(url);
                }
            }
        });
    });

    view! {
        <div class="space-y-6">
            // Header
            <div>
                <h1 class="text-3xl font-bold text-gray-900 mb-2">
                    "Sube tus tickets de Mercadona"
                </h1>
                <p class="text-gray-600">
                    "Analiza tus compras y descubre patrones de consumo. Puedes subir múltiples archivos a la vez."
                </p>
            </div>

            // Upload card
            <Card class="animate-slide-up".to_string()>
                <div class="space-y-6">
                    // Drop zone
                    <div
                        class="relative border-3 border-dashed border-gray-300 rounded-xl p-8 text-center hover:border-primary-400 transition-colors cursor-pointer group"
                        on:click=trigger_file_input
                    >
                        <input
                            node_ref=file_input_ref
                            type="file"
                            accept="image/*,.pdf"
                            class="hidden"
                            multiple=true
                            on:change=handle_file_select
                        />

                        <div class="space-y-4">
                            <div class="flex justify-center">
                                <div class="w-20 h-20 bg-primary-100 rounded-full flex items-center justify-center group-hover:bg-primary-200 transition-colors">
                                    <svg class="w-10 h-10 text-primary-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12"></path>
                                    </svg>
                                </div>
                            </div>
                            <div>
                                <p class="text-lg font-semibold text-gray-900 mb-2">
                                    "Arrastra tus tickets aquí o haz clic para seleccionar"
                                </p>
                                <p class="text-sm text-gray-600">
                                    "Soportamos PDF e imágenes (JPG, PNG)"
                                </p>
                            </div>
                        </div>
                    </div>

                    // File List
                    {move || if !files.get().is_empty() {
                        view! {
                            <div class="space-y-3">
                                <div class="flex justify-between items-center">
                                    <h3 class="font-medium text-gray-900">"Archivos seleccionados (" {move || files.get().len()} ")"</h3>
                                    {move || {
                                        let has_success = files.get().iter().any(|f| f.status == UploadStatus::Success);
                                        if has_success {
                                            view! {
                                                <button 
                                                    class="text-sm text-primary-600 hover:text-primary-700 font-medium"
                                                    on:click=clear_completed
                                                >
                                                    "Limpiar completados"
                                                </button>
                                            }.into_view()
                                        } else {
                                            view! { <div></div> }.into_view()
                                        }
                                    }}
                                </div>
                                <div class="grid gap-3">
                                    <For
                                        each=move || files.get()
                                        key=|f| f.id.clone()
                                        children=move |file| {
                                            let id = file.id.clone();
                                            let status = file.status.clone();
                                            let error = file.error.clone();
                                            let result = file.result.clone();
                                            
                                            view! {
                                                <div class="flex items-center p-3 bg-gray-50 rounded-lg border border-gray-200 group hover:border-gray-300 transition-colors">
                                                    // Preview/Icon
                                                    <div class="flex-shrink-0 w-12 h-12 bg-white rounded-lg border border-gray-200 flex items-center justify-center overflow-hidden mr-4">
                                                        {if let Some(url) = file.preview_url {
                                                            view! { <img src=url class="w-full h-full object-cover" /> }.into_view()
                                                        } else {
                                                            view! {
                                                                <svg class="w-6 h-6 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"></path>
                                                                </svg>
                                                            }.into_view()
                                                        }}
                                                    </div>
                                                    
                                                    // Info
                                                    <div class="flex-grow min-w-0 mr-4">
                                                        <p class="text-sm font-medium text-gray-900 truncate">
                                                            {file.file.name()}
                                                        </p>
                                                        <div class="text-xs mt-1">
                                                            {match status {
                                                                UploadStatus::Pending => view! { <span class="text-gray-500">"Pendiente"</span> }.into_view(),
                                                                UploadStatus::Processing => view! { <span class="text-blue-600 animate-pulse">"Procesando..."</span> }.into_view(),
                                                                UploadStatus::Success => {
                                                                    if let Some(res) = result {
                                                                        if let Some(ingestion) = res.ingestion {
                                                                            view! { 
                                                                                <span class="text-green-600 font-medium">
                                                                                    {format!("Guardado: {:.2}€ ({} prods)", res.ocr.total.unwrap_or(0.0), ingestion.productos_insertados)}
                                                                                </span> 
                                                                            }.into_view()
                                                                        } else {
                                                                            view! { <span class="text-yellow-600">"Procesado (Solo OCR)"</span> }.into_view()
                                                                        }
                                                                    } else {
                                                                        view! { <span class="text-green-600">"Completado"</span> }.into_view()
                                                                    }
                                                                },
                                                                UploadStatus::Error => view! { <span class="text-red-600">{error.unwrap_or_default()}</span> }.into_view(),
                                                            }}
                                                        </div>
                                                    </div>

                                                    // Status Icon / Action
                                                    <div class="flex-shrink-0">
                                                        {match status {
                                                            UploadStatus::Pending => view! {
                                                                <button 
                                                                    class="p-1 text-gray-400 hover:text-red-500 transition-colors"
                                                                    on:click=move |_| remove_file(id.clone())
                                                                    title="Eliminar"
                                                                >
                                                                    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"></path>
                                                                    </svg>
                                                                </button>
                                                            }.into_view(),
                                                            UploadStatus::Processing => view! {
                                                                <svg class="w-5 h-5 text-blue-500 animate-spin" fill="none" viewBox="0 0 24 24">
                                                                    <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                                                    <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                                                </svg>
                                                            }.into_view(),
                                                            UploadStatus::Success => view! {
                                                                <svg class="w-6 h-6 text-green-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"></path>
                                                                </svg>
                                                            }.into_view(),
                                                            UploadStatus::Error => view! {
                                                                <button 
                                                                    class="p-1 text-red-500 hover:text-red-700 transition-colors"
                                                                    on:click=move |_| remove_file(id.clone())
                                                                    title="Eliminar"
                                                                >
                                                                    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"></path>
                                                                    </svg>
                                                                </button>
                                                            }.into_view(),
                                                        }}
                                                    </div>
                                                </div>
                                            }
                                        }
                                    />
                                </div>
                            </div>
                        }.into_view()
                    } else {
                        view! { <div></div> }.into_view()
                    }}

                    // Action buttons
                    <div class="flex gap-4">
                        {move || if !files.get().is_empty() {
                            let has_pending = files.get().iter().any(|f| f.status == UploadStatus::Pending || f.status == UploadStatus::Error);
                            
                            view! {
                                <>
                                    <Button
                                        full_width=true
                                        loading=processing.get()
                                        disabled=processing.get() || !has_pending
                                        on:click=handle_process_all
                                    >
                                        <svg class="w-5 h-5 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"></path>
                                        </svg>
                                        {move || if processing.get() { "Procesando..." } else { "Procesar tickets pendientes" }}
                                    </Button>
                                </>
                            }.into_view()
                        } else {
                            view! { <div></div> }.into_view()
                        }}
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
