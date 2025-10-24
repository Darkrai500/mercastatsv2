use leptos::*;
use web_sys::HtmlInputElement;

#[component]
pub fn Input(
    /// Input label
    #[prop(optional)]
    label: Option<String>,
    /// Input placeholder
    #[prop(default = "".to_string())]
    placeholder: String,
    /// Input type
    #[prop(default = "text".to_string())]
    input_type: String,
    /// Input value signal
    value: RwSignal<String>,
    /// Error message
    #[prop(optional)]
    error: Option<String>,
    /// Required field
    #[prop(default = false)]
    required: bool,
    /// Disabled state
    #[prop(default = false)]
    disabled: bool,
    /// Input name
    #[prop(optional)]
    name: Option<String>,
    /// Autocomplete attribute
    #[prop(default = "off".to_string())]
    autocomplete: String,
) -> impl IntoView {
    let input_ref = create_node_ref::<html::Input>();

    let has_error = error.is_some();

    let input_classes = if has_error {
        "w-full px-4 py-3 rounded-lg border-2 border-red-300 focus:border-red-500 focus:ring-2 focus:ring-red-200 outline-none transition-all bg-white text-gray-900 placeholder-gray-400"
    } else {
        "w-full px-4 py-3 rounded-lg border-2 border-gray-200 focus:border-primary-500 focus:ring-2 focus:ring-primary-200 outline-none transition-all bg-white text-gray-900 placeholder-gray-400"
    };

    view! {
        <div class="w-full">
            {label.map(|l| view! {
                <label class="block text-sm font-medium text-gray-700 mb-2">
                    {l}
                    {if required {
                        view! { <span class="text-red-500 ml-1">"*"</span> }.into_view()
                    } else {
                        view! { <span></span> }.into_view()
                    }}
                </label>
            })}

            <input
                node_ref={input_ref}
                type={input_type}
                class={input_classes}
                placeholder={placeholder}
                value={move || value.get()}
                disabled={disabled}
                name={name.unwrap_or_default()}
                autocomplete={autocomplete}
                on:input=move |ev| {
                    let input_element = event_target::<HtmlInputElement>(&ev);
                    value.set(input_element.value());
                }
            />

            {error.map(|err| view! {
                <p class="mt-2 text-sm text-red-600 animate-fade-in">
                    {err}
                </p>
            })}
        </div>
    }
}
