use std::cell::Cell;
use std::rc::Rc;

use crate::api::google::{
    error_message, gmail_connect_url, gmail_disconnect, gmail_status, gmail_sync_now, take_flash,
    GmailStatus,
};
use crate::components::{Button, ButtonVariant, Card};
use gloo_timers::future::TimeoutFuture;
use leptos::*;

/// Panel de la sincronización automática con Gmail (conectar, estado, sincronizar, desconectar).
/// No se pinta nada si el servidor no tiene configurada la integración con Google.
#[component]
pub fn GmailSyncCard() -> impl IntoView {
    let (status, set_status) = create_signal(None::<GmailStatus>);
    // Aviso pendiente tras volver de Google: un fallo va en rojo, un éxito como información
    let flash = take_flash();
    let (error, set_error) = create_signal(
        flash.clone().filter(|f| f.is_error).map(|f| f.message),
    );
    let (notice, set_notice) = create_signal(
        flash.filter(|f| !f.is_error).map(|f| f.message),
    );
    let (busy, set_busy) = create_signal(false);

    // Mientras el componente esté en pantalla puede haber un bucle de consulta; al
    // desmontarlo se apaga para no tocar señales ya liberadas.
    let alive = Rc::new(Cell::new(true));
    let polling = Rc::new(Cell::new(false));
    {
        let alive = alive.clone();
        on_cleanup(move || alive.set(false));
    }

    // Consulta el estado y, mientras haya una sincronización en curso, sigue consultándolo.
    let poll = {
        let alive = alive.clone();
        let polling = polling.clone();
        move || {
            if polling.replace(true) {
                return;
            }
            let alive = alive.clone();
            let polling = polling.clone();
            spawn_local(async move {
                loop {
                    if !alive.get() {
                        break;
                    }
                    match gmail_status().await {
                        Ok(current) => {
                            let syncing = current.sincronizando;
                            set_status.try_set(Some(current));
                            if !syncing {
                                break;
                            }
                        }
                        Err(err) => {
                            set_error.try_set(Some(err));
                            break;
                        }
                    }
                    TimeoutFuture::new(3_000).await;
                }
                polling.set(false);
            });
        }
    };
    poll();

    let connect = Callback::new(move |_: ev::MouseEvent| {
        set_error.set(None);
        set_busy.set(true);
        spawn_local(async move {
            match gmail_connect_url().await {
                // Navegación completa: el flujo OAuth sale de la aplicación
                Ok(url) => {
                    if let Some(window) = web_sys::window() {
                        let _ = window.location().set_href(&url);
                    }
                }
                Err(err) => {
                    set_error.try_set(Some(err));
                    set_busy.try_set(false);
                }
            }
        });
    });

    let sync_now = {
        let poll = poll.clone();
        Callback::new(move |_: ev::MouseEvent| {
            set_error.set(None);
            set_notice.set(None);
            let poll = poll.clone();
            spawn_local(async move {
                match gmail_sync_now().await {
                    Ok(started) => {
                        if !started.iniciada {
                            set_notice.try_set(Some("Ya hay una sincronización en curso.".to_string()));
                        }
                        poll();
                    }
                    Err(err) => {
                        set_error.try_set(Some(err));
                    }
                }
            });
        })
    };

    let disconnect = {
        let poll = poll.clone();
        Callback::new(move |_: ev::MouseEvent| {
            let confirmed = web_sys::window()
                .and_then(|w| {
                    w.confirm_with_message(
                        "¿Desconectar Gmail? Dejaremos de importar tickets automáticamente. Los que ya se importaron se conservan.",
                    )
                    .ok()
                })
                .unwrap_or(false);
            if !confirmed {
                return;
            }
            set_error.set(None);
            set_busy.set(true);
            let poll = poll.clone();
            spawn_local(async move {
                match gmail_disconnect().await {
                    Ok(done) => {
                        if !done.revocado_en_google {
                            set_notice.try_set(Some(
                                "Gmail se ha desconectado, pero Google no confirmó la revocación. Puedes quitar el acceso en myaccount.google.com/permissions."
                                    .to_string(),
                            ));
                        } else {
                            set_notice.try_set(Some("Gmail desconectado.".to_string()));
                        }
                        poll();
                    }
                    Err(err) => {
                        set_error.try_set(Some(err));
                    }
                }
                set_busy.try_set(false);
            });
        })
    };

    view! {
        {move || {
            let Some(current) = status.get() else {
                return view! { <div></div> }.into_view();
            };
            if !current.disponible {
                return view! { <div></div> }.into_view();
            }

            let sender = current.remitente.clone().unwrap_or_default();
            let needs_reconnect = current.estado.as_deref() == Some("requiere_reconexion");

            view! {
                <Card class="animate-slide-up".to_string()>
                    <div class="flex items-start gap-4">
                        <div class="flex-shrink-0 w-12 h-12 bg-red-50 rounded-xl flex items-center justify-center">
                            <svg class="w-7 h-7 text-red-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 8l7.89 5.26a2 2 0 002.22 0L21 8M5 19h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"></path>
                            </svg>
                        </div>
                        <div class="flex-1 min-w-0 space-y-3">
                            {if !current.conectado {
                                view! {
                                    <div>
                                        <h2 class="text-lg font-semibold text-gray-900">
                                            "Importa tus tickets automáticamente desde Gmail"
                                        </h2>
                                        <p class="text-sm text-gray-600 mt-1">
                                            "Conecta tu cuenta de Google y Mercastats buscará los correos de "
                                            <span class="font-medium">{sender.clone()}</span>
                                            ", descargará el PDF de cada ticket y lo añadirá a tu historial. Se repite solo cada pocos minutos."
                                        </p>
                                        <p class="text-xs text-gray-500 mt-2">
                                            "Permiso de solo lectura: no puede enviar, borrar ni modificar correos, y solo se procesan los mensajes de ese remitente. Puedes desconectarlo cuando quieras."
                                        </p>
                                    </div>
                                }.into_view()
                            } else {
                                let imported = current.ultima_sync_importados;
                                let duplicated = current.ultima_sync_duplicados;
                                let failed = current.ultima_sync_fallidos;
                                let last_error = current
                                    .ultima_sync_error
                                    .as_deref()
                                    .map(error_message);
                                view! {
                                    <div>
                                        <div class="flex items-center gap-2">
                                            <h2 class="text-lg font-semibold text-gray-900">"Gmail conectado"</h2>
                                            {if needs_reconnect {
                                                view! { <span class="inline-flex px-2 py-0.5 rounded text-xs font-medium bg-amber-100 text-amber-800">"Requiere reconexión"</span> }.into_view()
                                            } else {
                                                view! { <span class="inline-flex px-2 py-0.5 rounded text-xs font-medium bg-green-100 text-green-800">"Sincronización automática"</span> }.into_view()
                                            }}
                                        </div>
                                        <p class="text-sm text-gray-600 mt-1">
                                            {current.google_email.clone().unwrap_or_default()}
                                            " · buscando correos de "
                                            <span class="font-medium">{sender.clone()}</span>
                                        </p>
                                        <p class="text-sm text-gray-500 mt-1">
                                            {if current.sincronizando {
                                                "Sincronizando ahora…".to_string()
                                            } else {
                                                match current.ultima_sync_fin.as_deref() {
                                                    Some(when) if current.ultima_sync_ok == Some(true) => format!(
                                                        "Última sincronización: {} · {}, {}{}",
                                                        format_date(when),
                                                        plural(imported, "nuevo", "nuevos"),
                                                        plural(duplicated, "ya estaba", "ya estaban"),
                                                        if failed > 0 {
                                                            format!(", {}", plural(failed, "no se pudo leer", "no se pudieron leer"))
                                                        } else {
                                                            String::new()
                                                        }
                                                    ),
                                                    Some(when) => format!("Último intento: {}", format_date(when)),
                                                    None => "Aún no se ha sincronizado.".to_string(),
                                                }
                                            }}
                                        </p>
                                        {last_error.map(|msg| view! {
                                            <p class="text-sm text-red-600 mt-1">{msg}</p>
                                        })}
                                    </div>
                                }.into_view()
                            }}

                            {move || notice.get().map(|msg| view! {
                                <p class="text-sm text-blue-700 bg-blue-50 border border-blue-100 rounded-lg px-3 py-2">{msg}</p>
                            })}
                            {move || error.get().map(|msg| view! {
                                <p class="text-sm text-red-700 bg-red-50 border border-red-100 rounded-lg px-3 py-2">{msg}</p>
                            })}

                            <div class="flex flex-wrap gap-3">
                                {if !current.conectado || needs_reconnect {
                                    view! {
                                        <Button loading=busy.get() disabled=busy.get() on:click=move |ev| connect.call(ev)>
                                            {if needs_reconnect { "Volver a conectar Gmail" } else { "Conectar Gmail" }}
                                        </Button>
                                    }.into_view()
                                } else {
                                    view! {
                                        <Button
                                            loading=current.sincronizando
                                            disabled=current.sincronizando || busy.get()
                                            on:click=move |ev| sync_now.call(ev)
                                        >
                                            {if current.sincronizando { "Sincronizando…" } else { "Sincronizar ahora" }}
                                        </Button>
                                    }.into_view()
                                }}
                                {current.conectado.then(|| view! {
                                    <Button
                                        variant=ButtonVariant::Ghost
                                        disabled=busy.get() || current.sincronizando
                                        on:click=move |ev| disconnect.call(ev)
                                    >
                                        "Desconectar"
                                    </Button>
                                })}
                            </div>
                        </div>
                    </div>
                </Card>
            }.into_view()
        }}
    }
}

fn plural(n: i32, one: &str, many: &str) -> String {
    format!("{} {}", n, if n == 1 { one } else { many })
}

/// Fecha RFC 3339 -> texto en la configuración regional del navegador.
fn format_date(rfc3339: &str) -> String {
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_str(rfc3339));
    if date.get_time().is_nan() {
        return rfc3339.to_string();
    }
    String::from(date.to_locale_string("es-ES", &wasm_bindgen::JsValue::UNDEFINED))
}
