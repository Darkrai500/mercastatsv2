use leptos::*;

#[component]
pub fn Card(
    /// Card content
    children: Children,
    /// Card title
    #[prop(optional)]
    title: Option<String>,
    /// Card padding
    #[prop(default = true)]
    padding: bool,
    /// Custom classes
    #[prop(default = "".to_string())]
    class: String,
) -> impl IntoView {
    let padding_class = if padding { "p-8" } else { "" };

    view! {
        <div class={format!("bg-white rounded-xl shadow-sm border border-gray-100 {} {}", padding_class, class)}>
            {title.map(|t| view! {
                <div class="mb-6">
                    <h2 class="text-2xl font-semibold text-gray-900">{t}</h2>
                </div>
            })}
            {children()}
        </div>
    }
}
