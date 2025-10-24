use leptos::*;

#[derive(Clone, Copy, PartialEq)]
pub enum ButtonVariant {
    Primary,
    Secondary,
    Outline,
    Ghost,
}

#[component]
pub fn Button(
    /// Button text
    children: Children,
    /// Button variant
    #[prop(default = ButtonVariant::Primary)]
    variant: ButtonVariant,
    /// Full width button
    #[prop(default = false)]
    full_width: bool,
    /// Disabled state
    #[prop(default = false)]
    disabled: bool,
    /// Loading state
    #[prop(default = false)]
    loading: bool,
    /// Button type
    #[prop(default = "button".to_string())]
    button_type: String,
) -> impl IntoView {
    let base_classes = "inline-flex items-center justify-center px-6 py-3 rounded-lg font-medium transition-all duration-200 focus:outline-none focus:ring-2 focus:ring-offset-2 disabled:opacity-50 disabled:cursor-not-allowed";

    let variant_classes = match variant {
        ButtonVariant::Primary => "bg-primary-600 text-white hover:bg-primary-700 focus:ring-primary-500 shadow-sm hover:shadow-md",
        ButtonVariant::Secondary => "bg-gray-200 text-gray-900 hover:bg-gray-300 focus:ring-gray-500",
        ButtonVariant::Outline => "border-2 border-primary-600 text-primary-600 hover:bg-primary-50 focus:ring-primary-500",
        ButtonVariant::Ghost => "text-gray-700 hover:bg-gray-100 focus:ring-gray-500",
    };

    let width_class = if full_width { "w-full" } else { "" };

    let classes = format!("{} {} {}", base_classes, variant_classes, width_class);

    view! {
        <button
            type={button_type}
            class={classes}
            disabled={disabled || loading}
        >
            {if loading {
                view! {
                    <svg class="animate-spin -ml-1 mr-3 h-5 w-5" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                    </svg>
                }.into_view()
            } else {
                view! { <span></span> }.into_view()
            }}
            {children()}
        </button>
    }
}
