use leptos::*;

#[component]
pub fn AIGeneratingLoader() -> impl IntoView {
    view! {
        <div class="w-full max-w-2xl mx-auto p-6 bg-white rounded-xl shadow-sm border border-gray-100">
            <div class="space-y-4">
                // Header shimmer
                <div class="h-8 bg-gray-100 rounded w-1/3 animate-pulse relative overflow-hidden">
                    <div class="absolute inset-0 -translate-x-full animate-[shimmer_2s_infinite] bg-gradient-to-r from-transparent via-white/50 to-transparent"></div>
                </div>
                
                // Text lines shimmer
                <div class="space-y-2">
                    <div class="h-4 bg-gray-100 rounded w-full animate-pulse relative overflow-hidden">
                        <div class="absolute inset-0 -translate-x-full animate-[shimmer_2s_infinite] bg-gradient-to-r from-transparent via-white/50 to-transparent"></div>
                    </div>
                    <div class="h-4 bg-gray-100 rounded w-5/6 animate-pulse relative overflow-hidden">
                        <div class="absolute inset-0 -translate-x-full animate-[shimmer_2s_infinite] bg-gradient-to-r from-transparent via-white/50 to-transparent"></div>
                    </div>
                    <div class="h-4 bg-gray-100 rounded w-4/6 animate-pulse relative overflow-hidden">
                        <div class="absolute inset-0 -translate-x-full animate-[shimmer_2s_infinite] bg-gradient-to-r from-transparent via-white/50 to-transparent"></div>
                    </div>
                </div>

                // Products shimmer
                <div class="grid grid-cols-1 md:grid-cols-2 gap-4 mt-8">
                    <div class="h-24 bg-gray-100 rounded-lg animate-pulse relative overflow-hidden">
                        <div class="absolute inset-0 -translate-x-full animate-[shimmer_2s_infinite] bg-gradient-to-r from-transparent via-white/50 to-transparent"></div>
                    </div>
                    <div class="h-24 bg-gray-100 rounded-lg animate-pulse relative overflow-hidden">
                        <div class="absolute inset-0 -translate-x-full animate-[shimmer_2s_infinite] bg-gradient-to-r from-transparent via-white/50 to-transparent"></div>
                    </div>
                </div>
            </div>
            
            <div class="mt-6 flex items-center justify-center text-sm text-gray-500 gap-2">
                <svg class="w-4 h-4 animate-spin" fill="none" viewBox="0 0 24 24">
                    <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                    <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                </svg>
                <span>"Analizando patrones de compra..."</span>
            </div>
        </div>
    }
}
