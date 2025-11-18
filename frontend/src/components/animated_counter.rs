use leptos::*;
use std::time::Duration;

#[component]
pub fn AnimatedCounter(
    /// Valor final a alcanzar
    target: f64,

    /// Duración de la animación en milisegundos
    #[prop(default = 1000)]
    duration: u64,

    /// Número de decimales a mostrar
    #[prop(default = 2)]
    decimals: usize,

    /// Prefijo (ej: "€", "$")
    #[prop(default = "")]
    prefix: String,

    /// Sufijo (ej: "€", "%")
    #[prop(default = "")]
    suffix: String,

    /// Clase CSS personalizada
    #[prop(default = "")]
    class: String,
) -> impl IntoView {
    let (current_value, set_current_value) = create_signal(0.0);
    let (is_animating, set_is_animating) = create_signal(true);

    create_effect(move |_| {
        if is_animating() && duration > 0 {
            let start_time = js_sys::Date::now();
            let target_time = start_time + duration as f64;

            let interval_id = set_interval_with_handle(
                move || {
                    let current_time = js_sys::Date::now();

                    if current_time >= target_time {
                        set_current_value(target);
                        set_is_animating(false);
                    } else {
                        let progress = (current_time - start_time) / (target_time - start_time);
                        let eased_progress = ease_out_cubic(progress);
                        let new_value = target * eased_progress;
                        set_current_value(new_value);
                    }
                },
                16, // ~60fps
            );

            on_cleanup(move || {
                interval_id.clear();
            });
        } else {
            set_current_value(target);
        }
    });

    let formatted_value = move || {
        let formatted = format!("{:.prec$}", current_value(), prec = decimals);
        format!("{}{}{}", prefix, formatted, suffix)
    };

    view! {
        <span class=format!("font-mono text-lg font-semibold text-gray-900 {} {}",
            if is_animating() { "animate-pulse-glow" } else { "" },
            class
        )>
            {formatted_value}
        </span>
    }
}

/// Easing function: cubic ease-out
fn ease_out_cubic(t: f64) -> f64 {
    let p = t - 1.0;
    p * p * p + 1.0
}

#[component]
pub fn KpiCard(
    /// Título de la KPI
    title: String,

    /// Valor a mostrar (se animará)
    value: f64,

    /// Número de decimales
    #[prop(default = 2)]
    decimals: usize,

    /// Prefijo (ej: "€")
    #[prop(default = "")]
    prefix: String,

    /// Sufijo (ej: "%")
    #[prop(default = "")]
    suffix: String,

    /// Porcentaje de cambio (opcional)
    #[prop(default = None)]
    trend: Option<f64>,

    /// Mostrar tendencia como positiva o negativa
    #[prop(default = true)]
    is_positive_trend: bool,

    /// Icono o elemento personalizado
    #[prop(default = "")]
    icon: String,

    /// Clase CSS personalizada
    #[prop(default = "")]
    class: String,

    /// Delay de animación en milisegundos
    #[prop(default = 0)]
    animation_delay: u64,
) -> impl IntoView {
    let trend_text = trend.map(|t| {
        let symbol = if t >= 0.0 { "↑" } else { "↓" };
        let abs_value = t.abs();
        format!("{} {:.1}%", symbol, abs_value)
    });

    let trend_color = trend.map(|t| {
        if is_positive_trend {
            if t >= 0.0 {
                "text-emerald-600" // green for positive
            } else {
                "text-red-600" // red for negative
            }
        } else {
            if t >= 0.0 {
                "text-red-600" // red for positive (bad)
            } else {
                "text-emerald-600" // green for negative (good)
            }
        }
    });

    view! {
        <div
            class=format!(
                "bg-white rounded-lg border border-gray-100 p-6 shadow-sm hover:shadow-md transition-shadow duration-300 animate-slide-up {} {}",
                class,
                match animation_delay {
                    0 => "",
                    100 => "delay-100",
                    200 => "delay-200",
                    300 => "delay-300",
                    400 => "delay-400",
                    500 => "delay-500",
                    _ => "",
                }
            )
        >
            <div class="flex items-start justify-between">
                <div class="flex-1">
                    <p class="text-xs font-medium text-gray-500 uppercase tracking-wider mb-2">
                        {title}
                    </p>

                    <div class="flex items-baseline gap-2">
                        <AnimatedCounter
                            target=value
                            decimals=decimals
                            prefix=prefix.clone()
                            suffix=suffix.clone()
                            duration=1200
                            class="text-2xl"
                        />
                    </div>

                    {trend_text.map(|text| {
                        view! {
                            <p class=format!("text-sm font-medium mt-2 {}", trend_color.unwrap_or("text-gray-600"))>
                                {text}
                            </p>
                        }
                    })}
                </div>

                {if !icon.is_empty() {
                    view! {
                        <div class="text-2xl ml-4">
                            {icon}
                        </div>
                    }
                    .into_view()
                } else {
                    view! { <></> }.into_view()
                }}
            </div>
        </div>
    }
}
