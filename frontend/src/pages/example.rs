use leptos::*;

/// Página de ejemplo para demostrar la navegación en el Dashboard
#[component]
pub fn ExamplePage() -> impl IntoView {
    view! {
        <div class="space-y-6">
            // Header
            <div>
                <h1 class="text-3xl font-bold text-gray-900 mb-2">
                    "Estadísticas"
                </h1>
                <p class="text-gray-600">
                    "Esta es una página de ejemplo para demostrar el sistema de navegación del Dashboard"
                </p>
            </div>

            // Cards de ejemplo
            <div class="grid grid-cols-1 md:grid-cols-3 gap-6">
                // Card 1
                <div class="bg-white border border-gray-200 rounded-lg p-6 hover:shadow-md transition-shadow">
                    <div class="flex items-center gap-4 mb-4">
                        <div class="flex items-center justify-center w-12 h-12 bg-blue-100 rounded-lg">
                            <svg class="w-6 h-6 text-blue-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8c-1.657 0-3 .895-3 2s1.343 2 3 2 3 .895 3 2-1.343 2-3 2m0-8c1.11 0 2.08.402 2.599 1M12 8V7m0 1v8m0 0v1m0-1c-1.11 0-2.08-.402-2.599-1M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path>
                            </svg>
                        </div>
                        <div>
                            <p class="text-sm text-gray-600">"Gasto total"</p>
                            <p class="text-2xl font-bold text-gray-900">"€1,234.56"</p>
                        </div>
                    </div>
                    <div class="flex items-center gap-2 text-sm">
                        <span class="text-green-600 font-medium">"+12.5%"</span>
                        <span class="text-gray-500">"vs. mes anterior"</span>
                    </div>
                </div>

                // Card 2
                <div class="bg-white border border-gray-200 rounded-lg p-6 hover:shadow-md transition-shadow">
                    <div class="flex items-center gap-4 mb-4">
                        <div class="flex items-center justify-center w-12 h-12 bg-green-100 rounded-lg">
                            <svg class="w-6 h-6 text-green-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"></path>
                            </svg>
                        </div>
                        <div>
                            <p class="text-sm text-gray-600">"Tickets"</p>
                            <p class="text-2xl font-bold text-gray-900">"42"</p>
                        </div>
                    </div>
                    <div class="flex items-center gap-2 text-sm">
                        <span class="text-green-600 font-medium">"+5"</span>
                        <span class="text-gray-500">"este mes"</span>
                    </div>
                </div>

                // Card 3
                <div class="bg-white border border-gray-200 rounded-lg p-6 hover:shadow-md transition-shadow">
                    <div class="flex items-center gap-4 mb-4">
                        <div class="flex items-center justify-center w-12 h-12 bg-purple-100 rounded-lg">
                            <svg class="w-6 h-6 text-purple-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 12l3-3 3 3 4-4M8 21l4-4 4 4M3 4h18M4 4h16v12a1 1 0 01-1 1H5a1 1 0 01-1-1V4z"></path>
                            </svg>
                        </div>
                        <div>
                            <p class="text-sm text-gray-600">"Promedio/ticket"</p>
                            <p class="text-2xl font-bold text-gray-900">"€29.39"</p>
                        </div>
                    </div>
                    <div class="flex items-center gap-2 text-sm">
                        <span class="text-red-600 font-medium">"-2.1%"</span>
                        <span class="text-gray-500">"vs. mes anterior"</span>
                    </div>
                </div>
            </div>

            // Información adicional
            <div class="bg-gradient-to-br from-primary-50 to-blue-50 border border-primary-200 rounded-lg p-6">
                <div class="flex items-start gap-4">
                    <div class="flex items-center justify-center w-10 h-10 bg-primary-600 rounded-lg flex-shrink-0">
                        <svg class="w-6 h-6 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path>
                        </svg>
                    </div>
                    <div class="flex-1">
                        <h3 class="font-semibold text-gray-900 mb-1">
                            "Página de ejemplo"
                        </h3>
                        <p class="text-sm text-gray-700 mb-3">
                            "Esta es una página de demostración. En el futuro, aquí se mostrarán estadísticas reales de tus compras, gráficos interactivos, y análisis detallados de tu consumo."
                        </p>
                        <div class="flex flex-wrap gap-2">
                            <span class="px-3 py-1 bg-white rounded-md text-xs font-medium text-gray-700 border border-gray-200">
                                "Gráficos"
                            </span>
                            <span class="px-3 py-1 bg-white rounded-md text-xs font-medium text-gray-700 border border-gray-200">
                                "Tendencias"
                            </span>
                            <span class="px-3 py-1 bg-white rounded-md text-xs font-medium text-gray-700 border border-gray-200">
                                "Análisis"
                            </span>
                        </div>
                    </div>
                </div>
            </div>

            // Lista de funcionalidades futuras
            <div class="bg-white border border-gray-200 rounded-lg p-6">
                <h2 class="text-xl font-semibold text-gray-900 mb-4">
                    "Funcionalidades próximamente"
                </h2>
                <div class="space-y-3">
                    <div class="flex items-start gap-3">
                        <div class="flex items-center justify-center w-5 h-5 bg-primary-100 rounded mt-0.5">
                            <svg class="w-3 h-3 text-primary-600" fill="currentColor" viewBox="0 0 20 20">
                                <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"></path>
                            </svg>
                        </div>
                        <div>
                            <p class="font-medium text-gray-900">"Gráficos de evolución de gastos"</p>
                            <p class="text-sm text-gray-600">"Visualiza cómo cambian tus gastos mes a mes"</p>
                        </div>
                    </div>
                    <div class="flex items-start gap-3">
                        <div class="flex items-center justify-center w-5 h-5 bg-primary-100 rounded mt-0.5">
                            <svg class="w-3 h-3 text-primary-600" fill="currentColor" viewBox="0 0 20 20">
                                <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"></path>
                            </svg>
                        </div>
                        <div>
                            <p class="font-medium text-gray-900">"Productos más comprados"</p>
                            <p class="text-sm text-gray-600">"Descubre qué productos compras más frecuentemente"</p>
                        </div>
                    </div>
                    <div class="flex items-start gap-3">
                        <div class="flex items-center justify-center w-5 h-5 bg-primary-100 rounded mt-0.5">
                            <svg class="w-3 h-3 text-primary-600" fill="currentColor" viewBox="0 0 20 20">
                                <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"></path>
                            </svg>
                        </div>
                        <div>
                            <p class="font-medium text-gray-900">"Análisis de inflación personal"</p>
                            <p class="text-sm text-gray-600">"Calcula cómo te afecta la inflación según tus compras"</p>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}
