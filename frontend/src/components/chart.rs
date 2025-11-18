use js_sys::Object;
use leptos::*;
use serde_json::json;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;
use web_sys::document;

#[wasm_bindgen]
extern "C" {
    pub type ApexCharts;

    #[wasm_bindgen(constructor)]
    pub fn new(el: web_sys::HtmlElement, options: JsValue) -> ApexCharts;

    #[wasm_bindgen(method)]
    pub fn render(this: &ApexCharts) -> js_sys::Promise;

    #[wasm_bindgen(method)]
    pub fn updateOptions(this: &ApexCharts, options: JsValue, redraw: bool, animate: bool) -> js_sys::Promise;

    #[wasm_bindgen(method)]
    pub fn updateSeries(this: &ApexCharts, series: JsValue, animate: bool) -> js_sys::Promise;

    #[wasm_bindgen(method)]
    pub fn destroy(this: &ApexCharts);
}

#[derive(Clone, Debug)]
pub struct ChartSeriesData {
    pub name: String,
    pub data: Vec<f64>,
}

#[derive(Clone, Debug)]
pub enum ChartType {
    Area,
    Bar,
    BarHorizontal,
    Line,
    Pie,
    Donut,
}

impl ChartType {
    fn as_str(&self) -> &str {
        match self {
            ChartType::Area => "area",
            ChartType::Bar => "bar",
            ChartType::BarHorizontal => "barHorizontal",
            ChartType::Line => "line",
            ChartType::Pie => "pie",
            ChartType::Donut => "donut",
        }
    }
}

#[component]
pub fn Chart(
    /// ID único para el contenedor del gráfico
    id: String,

    /// Tipo de gráfico (Area, Bar, Line, etc.)
    chart_type: ChartType,

    /// Datos a mostrar
    series: Vec<ChartSeriesData>,

    /// Etiquetas del eje X (para algunos gráficos)
    #[prop(default = vec![])]
    categories: Vec<String>,

    /// Altura del gráfico en píxeles
    #[prop(default = 350)]
    height: i32,

    /// Título del gráfico (opcional)
    #[prop(default = "")]
    title: String,

    /// Subtítulo (opcional)
    #[prop(default = "")]
    subtitle: String,

    /// Clase CSS personalizada
    #[prop(default = "")]
    class: String,
) -> impl IntoView {
    let container_id = id.clone();
    let chart_type_str = chart_type.as_str().to_string();

    create_effect(move |_| {
        // Esperar a que el DOM esté listo
        let _ = set_timeout(
            move || {
                if let Some(container) = document().get_element_by_id(&container_id) {
                    let container_el = container
                        .dyn_into::<web_sys::HtmlElement>()
                        .unwrap_or_else(|_| panic!("El contenedor no es un HtmlElement"));

                    // Construir opciones del gráfico
                    let mut options = json!({
                        "chart": {
                            "type": chart_type_str,
                            "height": height,
                            "sparkline": {
                                "enabled": false
                            },
                            "toolbar": {
                                "show": true,
                                "tools": {
                                    "download": true,
                                    "selection": true,
                                    "zoom": true,
                                    "zoomin": true,
                                    "zoomout": true,
                                    "pan": true,
                                    "reset": true
                                }
                            },
                            "animations": {
                                "enabled": true,
                                "easing": "easeinout",
                                "speed": 800,
                                "animateGradually": {
                                    "enabled": true,
                                    "delay": 150
                                },
                                "dynamicAnimation": {
                                    "enabled": true,
                                    "speed": 150
                                }
                            }
                        },
                        "series": series
                            .iter()
                            .map(|s| json!({
                                "name": s.name,
                                "data": s.data,
                            }))
                            .collect::<Vec<_>>(),
                        "xaxis": {
                            "categories": categories
                        },
                        "stroke": {
                            "curve": "smooth",
                            "width": 2
                        },
                        "colors": [
                            "#0ea5e9",  // primary-500
                            "#d946ef",  // accent-500
                            "#10b981",  // emerald-500
                            "#f59e0b",  // amber-500
                            "#8b5cf6",  // violet-500
                        ],
                        "grid": {
                            "borderColor": "#e2e8f0",
                            "strokeDashArray": 3,
                            "xaxis": {
                                "lines": {
                                    "show": false
                                }
                            }
                        },
                        "theme": {
                            "mode": "light",
                            "palette": "palette1"
                        },
                        "responsive": [{
                            "breakpoint": 768,
                            "options": {
                                "chart": {
                                    "height": 250
                                }
                            }
                        }]
                    });

                    // Añadir título si existe
                    if !title.is_empty() {
                        options["title"] = json!({
                            "text": title,
                            "align": "left",
                            "style": {
                                "fontSize": "16px",
                                "fontWeight": 600,
                                "color": "#1f2937"
                            }
                        });
                    }

                    // Añadir subtítulo si existe
                    if !subtitle.is_empty() {
                        options["subtitle"] = json!({
                            "text": subtitle,
                            "align": "left",
                            "style": {
                                "fontSize": "12px",
                                "color": "#6b7280"
                            }
                        });
                    }

                    // Crear el gráfico
                    match JsValue::from_serde(&options) {
                        Ok(options_js) => {
                            let chart = ApexCharts::new(container_el, options_js);
                            let _ = chart.render();

                            // Evitar memory leak guardando referencia en web_sys
                            // (Idealmente guardaríamos esto en un RwSignal para poder destruirlo después)
                        }
                        Err(e) => {
                            leptos::logging::error!("Error serializando opciones del gráfico:", e);
                        }
                    }
                }
            },
            100,
        );
    });

    view! {
        <div
            class=format!("w-full bg-white rounded-lg shadow-sm border border-gray-100 overflow-hidden {} animate-fade-in", class)
        >
            <div id=id class="w-full" style=format!("min-height: {}px", height) />
        </div>
    }
}
