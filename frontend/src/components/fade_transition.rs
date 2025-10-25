use leptos::*;

/// Componente wrapper que envuelve subpáginas con transiciones suaves
/// Utiliza fade + slide up para un efecto elegante y minimalista
#[component]
pub fn FadeTransition(
    #[prop(into)] children: Children,
) -> impl IntoView {
    view! {
        <div class="animate-fade-in">
            {children()}
        </div>
    }
}

/// Componente alternativo con slide desde la izquierda (para futuras variaciones)
#[component]
pub fn SlideInTransition(
    #[prop(into)] children: Children,
) -> impl IntoView {
    view! {
        <div class="animate-slide-up">
            {children()}
        </div>
    }
}
