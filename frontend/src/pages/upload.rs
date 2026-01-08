use crate::api::tickets::{process_ticket_ocr, ProcessTicketResponse};
use crate::components::{Button, ButtonVariant, Card};
use leptos::*;
use std::collections::HashSet;
use wasm_bindgen::JsCast;
use web_sys::{File, FileList, HtmlInputElement};

#[derive(Clone, PartialEq)]
enum UploadStatus {
    Pending,
    Processing,
    Review,
    Ingesting,
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
    let (is_demo, set_is_demo) = create_signal(false);
    let (upload_error, set_upload_error) = create_signal(None::<String>);
    let (review_modal_open, set_review_modal_open) = create_signal(false);

    let review_files = create_memo(move |_| {
        files.get()
            .into_iter()
            .filter(|f| f.status == UploadStatus::Review)
            .collect::<Vec<_>>()
    });

    // Obtener email y estado demo del usuario de localStorage
    create_effect(move |_| {
        if let Some(window) = web_sys::window() {
            if let Ok(Some(storage)) = window.local_storage() {
                if let Ok(Some(email)) = storage.get_item("user_email") {
                    set_user_email.set(Some(email));
                }
                if let Ok(Some(demo_str)) = storage.get_item("user_is_demo") {
                    set_is_demo.set(demo_str == "true");
                }
            }
        }
    });

    let file_input_ref = create_node_ref::<leptos::html::Input>();

    let build_file_status = |file: File| {
        let id = format!("{}-{}", file.name(), js_sys::Date::now());
        let mut preview_url = None;
        if file.type_().starts_with("image/") {
            preview_url = create_object_url(&file).ok();
        }

        FileStatus {
            id,
            file,
            status: UploadStatus::Pending,
            result: None,
            error: None,
            preview_url,
        }
    };

    // Helper to check if batch is complete and reset processing state
    let check_and_reset_batch = move || {
        let batch_complete = files.with(|files| {
            processing_batch.with(|batch_ids| {
                if batch_ids.is_empty() {
                    return true;
                }
                batch_ids.iter().all(|batch_id| {
                    files.iter()
                        .find(|f| &f.id == batch_id)
                        .map(|f| matches!(f.status, UploadStatus::Success | UploadStatus::Error | UploadStatus::Review))
                        .unwrap_or(true) // If file was removed, consider it done
                })
            })
        });
        
        if batch_complete {
            set_processing.set(false);
            set_processing_batch.set(HashSet::new());
        }
    };

    let refresh_review_modal = move || {
        let has_review = files.with(|items| items.iter().any(|f| f.status == UploadStatus::Review));
        set_review_modal_open.set(has_review);
    };

    let add_files_from_list = {
        let build_file_status_fn = build_file_status;
        move |file_list: FileList| {
            let mut new_files = Vec::new();
            let mut rejected = Vec::new();

            for i in 0..file_list.length() {
                if let Some(file) = file_list.get(i) {
                    if is_allowed_file(&file) {
                        new_files.push(build_file_status_fn(file));
                    } else {
                        rejected.push(file.name());
                    }
                }
            }

            if !new_files.is_empty() {
                set_files.update(|current| current.extend(new_files));
                set_upload_error.set(None);
            }

            if !rejected.is_empty() {
                set_upload_error.set(Some(format!(
                    "Formato no permitido: {}. Solo se aceptan imágenes o PDF.",
                    rejected.join(", ")
                )));
            }
        }
    };

    let handle_file_select = move |ev: web_sys::Event| {
        let target = event_target::<HtmlInputElement>(&ev);
        if let Some(file_list) = target.files() {
            add_files_from_list(file_list);

            // Reset input so same files can be selected again if needed
            if let Some(input) = file_input_ref.get() {
                input.set_value("");
            }
        }
    };

    let handle_drop = move |ev: web_sys::DragEvent| {
        ev.prevent_default();
        if let Some(data_transfer) = ev.data_transfer() {
            if let Some(file_list) = data_transfer.files() {
                add_files_from_list(file_list);
            }
        }
    };

    let handle_drag_over = move |ev: web_sys::DragEvent| {
        ev.prevent_default();
    };

    let handle_paste = move |ev: web_sys::Event| {
        if let Ok(clip_ev) = ev.dyn_into::<web_sys::ClipboardEvent>() {
            if let Some(data) = clip_ev.clipboard_data() {
                if let Some(files) = data.files() {
                    add_files_from_list(files);
                }
            }
        }
    };

    let process_file = move |file_status: FileStatus, ingest: bool| {
        if is_demo.get() {
            set_upload_error.set(Some("Como usuario demo no puedes insertar tickets. Crea un usuario para poder hacerlo.".to_string()));
            return;
        }

        let id = file_status.id.clone();
        let file_clone = file_status.file.clone();
        
        spawn_local(async move {
            // Update status to processing or ingesting
            set_files.update(|files| {
                if let Some(f) = files.iter_mut().find(|f| f.id == id) {
                    f.status = if ingest {
                        UploadStatus::Ingesting
                    } else {
                        UploadStatus::Processing
                    };
                    f.error = None;
                }
            });

            match process_ticket_ocr(file_clone, ingest).await {
                Ok(response) => {
                    set_files.update(|files| {
                        if let Some(f) = files.iter_mut().find(|f| f.id == id) {
                            f.status = if ingest {
                                UploadStatus::Success
                            } else {
                                UploadStatus::Review
                            };
                            f.result = Some(response.clone());
                        }
                    });

                    refresh_review_modal();
                }
                Err(err) => {
                    let error_msg = if err.contains("ya existe") || err.contains("Conflict") {
                        format!("Duplicado: {}", err)
                    } else {
                        err
                    };
                    
                    set_files.update(|files| {
                        if let Some(f) = files.iter_mut().find(|f| f.id == id) {
                            f.status = UploadStatus::Error;
                            f.error = Some(error_msg);
                        }
                    });

                    refresh_review_modal();
                }
            }
            
            // Check if batch is complete and reset processing state
            if !ingest {
                check_and_reset_batch();
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
            process_file(file, false);
        }
    };

    let confirm_file = move |id: String| {
        if let Some(target) = files
            .get()
            .into_iter()
            .find(|f| f.id == id && matches!(f.status, UploadStatus::Review | UploadStatus::Error))
        {
            process_file(target, true);
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
        
        // Remove from processing batch and check if batch is now complete
        set_processing_batch.update(|batch| {
            batch.remove(&id);
        });
        refresh_review_modal();
        check_and_reset_batch();
    };

    let clear_completed = move |_| {
        // Collect IDs of completed files before removing them
        let completed_ids: Vec<String> = files.with(|files| {
            files.iter()
                .filter(|f| f.status == UploadStatus::Success)
                .map(|f| f.id.clone())
                .collect()
        });
        
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
        
        // Remove completed files from processing batch and check if batch is now complete
        set_processing_batch.update(|batch| {
            for id in completed_ids {
                batch.remove(&id);
            }
        });
        check_and_reset_batch();
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
                        on:dragover=handle_drag_over
                        on:drop=handle_drop
                        on:paste=handle_paste
                        tabindex="0"
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

                        {move || upload_error.get().map(|msg| view! {
                            <p class="mt-3 text-sm text-red-600">{msg}</p>
                        })}
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
                                                                UploadStatus::Review => view! { <span class="text-amber-600 font-medium">"Listo para confirmar"</span> }.into_view(),
                                                                UploadStatus::Ingesting => view! { <span class="text-blue-600 animate-pulse">"Guardando..."</span> }.into_view(),
                                                                UploadStatus::Success => {
                                                                    if let Some(ref res) = result {
                                                                        if let Some(ingestion) = &res.ingestion {
                                                                            view! { 
                                                                                <span class="text-green-600 font-medium">
                                                                                    {format!("Guardado: {:.2}€ ({} prods)", res.ocr.total.unwrap_or(0.0), ingestion.productos.len())}
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
                                                        {match &result {
                                                            Some(res) if !res.ocr.warnings.is_empty() => {
                                                                view! {
                                                                    <p class="text-xs text-amber-600 mt-1">
                                                                        {format!("Avisos OCR: {}", res.ocr.warnings.join(" | "))}
                                                                    </p>
                                                                }.into_view()
                                                            }
                                                            _ => view! { <div></div> }.into_view(),
                                                        }}
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
                                                            UploadStatus::Review => view! {
                                                                <div class="flex items-center gap-2">
                                                                    <button
                                                                        class="p-1 text-amber-600 hover:text-amber-700 transition-colors"
                                                                        on:click=move |_| set_review_modal_open.set(true)
                                                                        title="Revisar deteccion"
                                                                    >
                                                                        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                                                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z" />
                                                                        </svg>
                                                                    </button>
                                                                    <button 
                                                                        class="p-1 text-gray-400 hover:text-red-500 transition-colors"
                                                                        on:click=move |_| remove_file(id.clone())
                                                                        title="Eliminar"
                                                                    >
                                                                        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"></path>
                                                                        </svg>
                                                                    </button>
                                                                </div>
                                                            }.into_view(),
                                                            UploadStatus::Ingesting => view! {
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
                            let review_count = files.get().iter().filter(|f| f.status == UploadStatus::Review).count();
                            
                            view! {
                                <>
                                    {move || if review_count > 0 {
                                        view! {
                                            <Button
                                                variant=ButtonVariant::Secondary
                                                on:click=move |_| set_review_modal_open.set(true)
                                            >
                                                {format!("Revisar detecciones ({})", review_count)}
                                            </Button>
                                        }.into_view()
                                    } else {
                                        view! { <div></div> }.into_view()
                                    }}
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

            {move || {
                let open = review_modal_open.get() && review_files.with(|f| !f.is_empty());

                view! {
                    <Show when=move || open>
                        <div class="fixed inset-0 z-40 flex items-center justify-center px-4 py-8 bg-gray-900/60 backdrop-blur-sm">
                            <div class="absolute inset-0 bg-gradient-to-b from-primary-50/40 to-white/10 pointer-events-none"></div>
                            <div class="relative w-full max-w-5xl bg-white rounded-2xl shadow-2xl border border-gray-100 overflow-hidden animate-scale-in">
                                <div class="flex items-start justify-between px-6 py-5 border-b border-gray-100">
                                    <div>
                                        <p class="text-xs uppercase tracking-[0.2em] text-primary-600 font-semibold">"Revision rapida"</p>
                                        <h3 class="text-2xl font-bold text-gray-900 mt-1">"Confirma antes de guardar"</h3>
                                        <p class="text-sm text-gray-500 mt-1">
                                            "Revisa total y productos detectados. Puedes descartar cualquier ticket sin afectar el resto."
                                        </p>
                                    </div>
                                    <button
                                        on:click=move |_| set_review_modal_open.set(false)
                                        class="text-gray-400 hover:text-gray-600 transition-colors"
                                        aria-label="Cerrar revision"
                                    >
                                        <svg class="h-6 w-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                                        </svg>
                                    </button>
                                </div>

                                <div class="p-6 space-y-4 max-h-[70vh] overflow-y-auto custom-scrollbar bg-gradient-to-b from-white to-gray-50">
                                    <For
                                        each=move || review_files.get()
                                        key=|f| f.id.clone()
                                        children=move |file| {
                                            let id = file.id.clone();
                                            let status = file.status.clone();
                                            let summary = file.result.clone().map(|r| r.ocr);
                                            let productos = summary.as_ref().map(|s| s.productos.clone()).unwrap_or_default();
                                            let warnings = summary.as_ref().map(|s| s.warnings.clone()).unwrap_or_default();
                                            let total_text = summary
                                                .as_ref()
                                                .and_then(|s| s.total)
                                                .map(|t| format!("{:.2} €", t))
                                                .unwrap_or_else(|| "Total no detectado".to_string());
                                            let productos_preview = productos.iter().take(4).cloned().collect::<Vec<_>>();
                                            let preview_len = productos_preview.len();

                                            view! {
                                                <div class="bg-white border border-gray-200 rounded-xl p-4 shadow-sm hover:shadow-md transition duration-200 animate-slide-up">
                                                    <div class="flex items-start gap-4">
                                                        <div class="w-14 h-14 rounded-lg border border-gray-200 overflow-hidden flex items-center justify-center bg-gray-50">
                                                            {if let Some(url) = file.preview_url.clone() {
                                                                view! { <img src=url class="w-full h-full object-cover" /> }.into_view()
                                                            } else {
                                                                view! {
                                                                    <svg class="w-7 h-7 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"></path>
                                                                    </svg>
                                                                }.into_view()
                                                            }}
                                                        </div>
                                                        <div class="flex-1 space-y-3">
                                                            <div class="flex items-start justify-between gap-2 flex-wrap">
                                                                <div>
                                                                    <p class="text-sm font-semibold text-gray-900">{file.file.name()}</p>
                                                                    <p class="text-xs text-gray-500">{format!("{} productos detectados", productos.len())}</p>
                                                                </div>
                                                                {match summary.as_ref().and_then(|s| s.processing_profile.clone()) {
                                                                    Some(profile) => view! {
                                                                        <span class="px-3 py-1 text-xs bg-primary-50 text-primary-700 rounded-full border border-primary-100">
                                                                            {format!("Pipeline: {}", profile)}
                                                                        </span>
                                                                    }.into_view(),
                                                                    None => view! { <div></div> }.into_view(),
                                                                }}
                                                            </div>

                                                            <div class="flex items-end justify-between gap-4 flex-wrap">
                                                                <div>
                                                                    <p class="text-xs font-semibold text-gray-600 uppercase tracking-wide">"Total detectado"</p>
                                                                    <p class="text-2xl font-bold text-gray-900 mt-1">{total_text}</p>
                                                                    {match summary.as_ref().and_then(|s| s.tienda.clone()) {
                                                                        Some(store) => view! {
                                                                            <p class="text-xs text-gray-500 mt-1">{store}</p>
                                                                        }.into_view(),
                                                                        None => view! { <div></div> }.into_view(),
                                                                    }}
                                                                </div>
                                                                <div class="flex gap-2">
                                                                    {
                                                                        let id_confirm = id.clone();
                                                                        view! {
                                                                            <Button
                                                                                loading=matches!(status, UploadStatus::Ingesting)
                                                                                disabled=matches!(status, UploadStatus::Ingesting)
                                                                                on:click=move |_| confirm_file(id_confirm.clone())
                                                                            >
                                                                                "Guardar ticket"
                                                                            </Button>
                                                                        }
                                                                    }
                                                                    {
                                                                        let id_remove = id.clone();
                                                                        view! {
                                                                            <Button
                                                                                variant=ButtonVariant::Secondary
                                                                                on:click=move |_| remove_file(id_remove.clone())
                                                                            >
                                                                                "Descartar"
                                                                            </Button>
                                                                        }
                                                                    }
                                                                </div>
                                                            </div>

                                                            <div class="mt-2">
                                                                <p class="text-xs font-semibold text-gray-600 uppercase tracking-wide mb-2">"Productos detectados"</p>
                                                                <div class="grid grid-cols-1 md:grid-cols-2 gap-2">
                                                                    <For
                                                                        each=move || productos_preview.clone()
                                                                        key=|prod| prod.nombre.clone()
                                                                        children=move |prod| {
                                                                            view! {
                                                                                <div class="flex items-center justify-between rounded-lg border border-gray-200 bg-gray-50 px-3 py-2">
                                                                                    <div>
                                                                                        <p class="text-sm font-medium text-gray-900 truncate">{prod.nombre.clone()}</p>
                                                                                        <p class="text-xs text-gray-500">
                                                                                            {format!("{:.3} {} • {:.2} €", prod.cantidad, prod.unidad, prod.precio_unitario)}
                                                                                        </p>
                                                                                    </div>
                                                                                    <p class="text-sm font-semibold text-gray-900">{format!("{:.2} €", prod.precio_total)}</p>
                                                                                </div>
                                                                            }
                                                                        }
                                                                    />
                                                                </div>
                                                                {
                                                                    if productos.len() > preview_len {
                                                                        view! {
                                                                            <p class="text-xs text-gray-500 mt-2">
                                                                                {format!("+ {} productos adicionales", productos.len() - preview_len)}
                                                                            </p>
                                                                        }.into_view()
                                                                    } else {
                                                                        view! { <div></div> }.into_view()
                                                                    }
                                                                }
                                                            </div>

                                                            {if !warnings.is_empty() {
                                                                view! {
                                                                    <div class="flex flex-wrap gap-2 mt-2">
                                                                        <For
                                                                            each=move || warnings.clone()
                                                                            key=|w| w.clone()
                                                                            children=move |w| {
                                                                                view! {
                                                                                    <span class="px-2 py-1 text-xs bg-amber-50 text-amber-700 rounded-md border border-amber-100">{w}</span>
                                                                                }
                                                                            }
                                                                        />
                                                                    </div>
                                                                }.into_view()
                                                            } else {
                                                                view! { <div></div> }.into_view()
                                                            }}
                                                        </div>
                                                    </div>
                                                </div>
                                            }
                                        }
                                    />
                                </div>
                            </div>
                        </div>
                    </Show>
                }
            }}
        </div>
    }
}

fn create_object_url(file: &File) -> Result<String, String> {
    web_sys::Url::create_object_url_with_blob(file)
        .map_err(|_| "Error al crear preview".to_string())
}

fn is_allowed_file(file: &File) -> bool {
    let mime = file.type_();
    if mime.starts_with("image/") || mime == "application/pdf" {
        return true;
    }

    let name = file.name().to_lowercase();
    let allowed_extensions = ["pdf", "png", "jpg", "jpeg", "webp", "heic", "heif"];
    allowed_extensions
        .iter()
        .any(|ext| name.ends_with(ext))
}

