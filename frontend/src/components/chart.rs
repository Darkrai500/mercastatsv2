use leptos::*;
use serde_json::json;
use std::time::Duration;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;

#[wasm_bindgen]
extern "C" {
    pub type ApexCharts;

    #[wasm_bindgen(constructor)]
    pub fn new(el: web_sys::HtmlElement, options: JsValue) -> ApexCharts;

    #[wasm_bindgen(method)]
    pub fn render(this: &ApexCharts) -> js_sys::Promise;

    #[wasm_bindgen(method)]
    pub fn updateOptions(
        this: &ApexCharts,
        options: JsValue,
        redraw: bool,
        animate: bool,
    ) -> js_sys::Promise;

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
            ChartType::BarHorizontal => "bar", // ApexCharts uses "bar" with horizontal option
            ChartType::Line => "line",
            ChartType::Pie => "pie",
            ChartType::Donut => "donut",
        }
    }

    fn is_horizontal(&self) -> bool {
        matches!(self, ChartType::BarHorizontal)
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
    #[prop(default = "".to_string())]
    title: String,

    /// Subtítulo (opcional)
    #[prop(default = "".to_string())]
    subtitle: String,

    /// Clase CSS personalizada
    #[prop(default = "".to_string())]
    class: String,
) -> impl IntoView {
    let container_id = id.clone();
    let chart_type_str = chart_type.as_str().to_string();
    let is_horizontal = chart_type.is_horizontal();

    create_effect(move |_| {
        // Clone variables para usarlas en el timeout
        let container_id_clone = container_id.clone();
        let chart_type_str_clone = chart_type_str.clone();
        let series_clone = series.clone();
        let categories_clone = categories.clone();
        let title_clone = title.clone();
        let subtitle_clone = subtitle.clone();

        // Esperar a que el DOM esté listo
        set_timeout(
            move || {
                if let Some(window) = web_sys::window() {
                    if let Some(document) = window.document() {
                        if let Some(container) = document.get_element_by_id(&container_id_clone) {
                            if let Ok(container_el) = container.dyn_into::<web_sys::HtmlElement>() {
                                // Construir opciones del gráfico
                                let mut options = json!({
                                    "chart": {
                                        "type": chart_type_str_clone,
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
                                    "plotOptions": {
                                        "bar": {
                                            "horizontal": is_horizontal,
                                            "borderRadius": 10,
                                            "borderRadiusApplication": "end",
                                            "distributed": is_horizontal,
                                            "dataLabels": {
                                                "position": "center"
                                            }
                                        }
                                    },
                                    "dataLabels": {
                                        "enabled": true,
                                        "style": {
                                            "fontSize": "12px",
                                            "fontWeight": 800,
                                            "colors": ["#ffffff"]
                                        },
                                        "offsetX": 0,
                                        "offsetY": 0
                                    },
                                    "series": series_clone
                                        .iter()
                                        .map(|s| json!({
                                            "name": s.name,
                                            "data": s.data,
                                        }))
                                        .collect::<Vec<_>>(),
                                    "xaxis": {
                                        "categories": categories_clone
                                    },
                                    "stroke": {
                                        "curve": "smooth",
                                        "width": 2
                                    },
                                    "colors": [
                                        "#38bdf8",  // sky-400
                                        "#c084fc",  // purple-400
                                        "#34d399",  // emerald-400
                                        "#f97316",  // orange-500
                                        "#fb7185",  // rose-400
                                    ],
                                    "fill": {
                                        "type": "gradient",
                                        "gradient": {
                                            "type": if is_horizontal { "horizontal" } else { "vertical" },
                                            "shade": "light",
                                            "shadeIntensity": 0.4,
                                            "opacityFrom": 0.95,
                                            "opacityTo": 0.85,
                                            "stops": [0, 50, 100]
                                        }
                                    },
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
                                if !title_clone.is_empty() {
                                    options["title"] = json!({
                                        "text": title_clone,
                                        "align": "left",
                                        "style": {
                                            "fontSize": "16px",
                                            "fontWeight": 600,
                                            "color": "#1f2937"
                                        }
                                    });
                                }

                                // Añadir subtítulo si existe
                                if !subtitle_clone.is_empty() {
                                    options["subtitle"] = json!({
                                        "text": subtitle_clone,
                                        "align": "left",
                                        "style": {
                                            "fontSize": "12px",
                                            "color": "#6b7280"
                                        }
                                    });
                                }

                                // Convertir JSON a JsValue usando into()
                                if let Ok(options_js) = JsValue::from_serde(&options) {
                                    let chart = ApexCharts::new(container_el, options_js);
                                    let _ = chart.render();
                                }
                            }
                        }
                    }
                }
            },
            Duration::from_millis(100),
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
