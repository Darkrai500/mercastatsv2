//! Revisión editable de la lectura de una foto de ticket.
//!
//! El OCR de una foto puede equivocarse en un dígito o en un nombre, así que antes de
//! guardar el usuario ve y corrige factura, fecha, total y productos. Los PDF digitales no
//! pasan por aquí: se leen de forma exacta y siguen su revisión de solo lectura.

use leptos::*;

use crate::api::tickets::{
    ConfirmPhotoTicketRequest, IvaBreakdown, OcrResponseSummary, TicketProduct,
};

/// Diferencia máxima entre la suma de productos y el total que acepta el backend
const TOLERANCIA_TOTAL: f64 = 0.10;

#[derive(Clone, Debug, PartialEq)]
pub struct DraftProduct {
    pub id: usize,
    pub nombre: String,
    pub cantidad: String,
    pub unidad: String,
    pub importe: String,
    /// Lo que leyó el OCR: si el usuario no toca cantidad ni importe se conserva su
    /// precio unitario e IVA en lugar de recalcularlos
    leido: Option<TicketProduct>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TicketDraft {
    pub numero_factura: String,
    /// `AAAA-MM-DD` (formato de `<input type="date">`)
    pub fecha: String,
    /// `HH:MM`
    pub hora: String,
    pub total: String,
    pub productos: Vec<DraftProduct>,
    next_id: usize,
    tienda: Option<String>,
    ubicacion: Option<String>,
    metodo_pago: Option<String>,
    numero_operacion: Option<String>,
    iva_desglose: Vec<IvaBreakdown>,
}

/// Importe escrito a la española o a la inglesa: "1,59", "1.59", "1,59 €"
pub fn parse_amount(value: &str) -> Option<f64> {
    let cleaned = value.replace('€', "").trim().replace(',', ".");
    cleaned.parse::<f64>().ok().filter(|n| n.is_finite())
}

fn euros(value: f64) -> String {
    format!("{:.2}", value).replace('.', ",")
}

fn quantity(value: f64) -> String {
    let text = format!("{:.3}", value);
    let text = text.trim_end_matches('0').trim_end_matches('.');
    text.replace('.', ",")
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// `06/10/2026` -> `2026-10-06`
fn iso_date(fecha: &str) -> Option<String> {
    let mut parts = fecha.split('/');
    let (d, m, y) = (parts.next()?, parts.next()?, parts.next()?);
    (d.len() == 2 && m.len() == 2 && y.len() == 4).then(|| format!("{y}-{m}-{d}"))
}

/// Formato de factura simplificada de Mercadona: `NNNN-NNN-NNNNNN`
fn valid_invoice(value: &str) -> bool {
    let groups: Vec<&str> = value.split('-').collect();
    groups.len() == 3
        && [4, 3, 6]
            .iter()
            .zip(&groups)
            .all(|(len, g)| g.len() == *len && g.chars().all(|c| c.is_ascii_digit()))
}

impl TicketDraft {
    pub fn from_summary(ocr: &OcrResponseSummary) -> Self {
        // fecha_hora llega como "AAAA-MM-DDTHH:MM:SS"; si no hay hora, solo "DD/MM/AAAA"
        let (fecha, hora) = match ocr.fecha_hora.as_deref() {
            Some(fh) if fh.len() >= 16 => (fh[..10].to_string(), fh[11..16].to_string()),
            _ => (
                ocr.fecha.as_deref().and_then(iso_date).unwrap_or_default(),
                String::new(),
            ),
        };

        let productos: Vec<DraftProduct> = ocr
            .productos
            .iter()
            .enumerate()
            .map(|(id, p)| DraftProduct {
                id,
                nombre: p.nombre.clone(),
                cantidad: quantity(p.cantidad),
                unidad: p.unidad.clone(),
                importe: euros(p.precio_total),
                leido: Some(p.clone()),
            })
            .collect();

        Self {
            numero_factura: ocr.numero_factura.clone().unwrap_or_default(),
            fecha,
            hora,
            total: ocr.total.map(euros).unwrap_or_default(),
            next_id: productos.len(),
            productos,
            tienda: ocr.tienda.clone(),
            ubicacion: ocr.ubicacion.clone(),
            metodo_pago: ocr.metodo_pago.clone(),
            numero_operacion: ocr.numero_operacion.clone(),
            iva_desglose: ocr.iva_desglose.clone(),
        }
    }

    fn add_product(&mut self) {
        self.productos.push(DraftProduct {
            id: self.next_id,
            nombre: String::new(),
            cantidad: "1".into(),
            unidad: "unidad".into(),
            importe: String::new(),
            leido: None,
        });
        self.next_id += 1;
    }

    fn product_mut(&mut self, id: usize) -> Option<&mut DraftProduct> {
        self.productos.iter_mut().find(|p| p.id == id)
    }

    /// Suma de los importes, si todos son números
    pub fn products_sum(&self) -> Option<f64> {
        self.productos
            .iter()
            .map(|p| parse_amount(&p.importe))
            .sum::<Option<f64>>()
            .map(round2)
    }

    /// Valida el borrador y lo convierte en la petición de guardado
    pub fn to_request(&self) -> Result<ConfirmPhotoTicketRequest, String> {
        let numero_factura = self.numero_factura.trim().to_string();
        if !valid_invoice(&numero_factura) {
            return Err("El número de factura debe tener el formato 1234-123-123456.".into());
        }
        if self.fecha.is_empty() || self.hora.is_empty() {
            return Err("Indica la fecha y la hora del ticket.".into());
        }
        let total = parse_amount(&self.total).ok_or("Indica el total del ticket.")?;
        if self.productos.is_empty() {
            return Err("El ticket debe tener al menos un producto.".into());
        }

        let mut productos = Vec::with_capacity(self.productos.len());
        for p in &self.productos {
            let nombre = p.nombre.trim().to_uppercase();
            if nombre.is_empty() {
                return Err("Hay un producto sin nombre.".into());
            }
            let cantidad = parse_amount(&p.cantidad)
                .filter(|c| *c > 0.0)
                .ok_or_else(|| format!("«{nombre}»: la cantidad no es válida."))?;
            let importe = parse_amount(&p.importe)
                .filter(|i| *i >= 0.0)
                .ok_or_else(|| format!("«{nombre}»: el importe no es válido."))?;

            let producto = match &p.leido {
                // Sin cambios en cantidad ni importe: se respeta lo leído del ticket
                Some(leido)
                    if (leido.cantidad - cantidad).abs() < 1e-9
                        && (leido.precio_total - importe).abs() < 1e-9 =>
                {
                    TicketProduct {
                        nombre,
                        ..leido.clone()
                    }
                }
                leido => TicketProduct {
                    nombre,
                    cantidad,
                    unidad: p.unidad.clone(),
                    precio_unitario: round2(importe / cantidad),
                    precio_total: importe,
                    descuento: 0.0,
                    iva_porcentaje: leido.as_ref().map(|l| l.iva_porcentaje).unwrap_or(0.0),
                    // 0 = que el backend lo calcule con el importe corregido
                    iva_importe: 0.0,
                },
            };
            productos.push(producto);
        }

        let suma = round2(productos.iter().map(|p| p.precio_total).sum());
        if (suma - total).abs() > TOLERANCIA_TOTAL {
            return Err(format!(
                "La suma de los productos ({} €) no coincide con el total ({} €).",
                euros(suma),
                euros(total)
            ));
        }

        Ok(ConfirmPhotoTicketRequest {
            file_name: String::new(),
            file_content_b64: String::new(),
            numero_factura,
            fecha_hora: format!("{}T{}:00", self.fecha, self.hora),
            total,
            tienda: self.tienda.clone(),
            ubicacion: self.ubicacion.clone(),
            metodo_pago: self.metodo_pago.clone(),
            numero_operacion: self.numero_operacion.clone(),
            productos,
            iva_desglose: self.iva_desglose.clone(),
        })
    }
}

const INPUT_CLASS: &str = "w-full rounded-lg border border-gray-300 px-3 py-2 text-sm text-gray-900 focus:border-primary-500 focus:outline-none focus:ring-2 focus:ring-primary-200";

/// Formulario editable de la lectura de una foto. Escribe directamente en `draft`.
#[component]
pub fn PhotoTicketEditor(draft: RwSignal<TicketDraft>) -> impl IntoView {
    let field = move |label: &'static str,
                      input_type: &'static str,
                      inputmode: &'static str,
                      placeholder: &'static str,
                      get: fn(&TicketDraft) -> String,
                      set: fn(&mut TicketDraft, String)| {
        view! {
            <label class="block">
                <span class="text-xs font-semibold text-gray-600 uppercase tracking-wide">{label}</span>
                <input
                    type=input_type
                    class=format!("mt-1 {INPUT_CLASS}")
                    placeholder=placeholder
                    inputmode=inputmode
                    prop:value=draft.with_untracked(get)
                    on:input=move |ev| draft.update(|d| set(d, event_target_value(&ev)))
                />
            </label>
        }
    };

    let product_row = move |id: usize| {
        let Some(initial) =
            draft.with_untracked(|d| d.productos.iter().find(|p| p.id == id).cloned())
        else {
            return view! { <tr></tr> }.into_view();
        };
        let unidad = (initial.unidad != "unidad").then(|| initial.unidad.clone());

        view! {
            <tr class="align-top">
                <td class="py-1 pr-2">
                    <input
                        type="text"
                        class=INPUT_CLASS
                        aria-label="Nombre del producto"
                        prop:value=initial.nombre.clone()
                        on:input=move |ev| draft.update(|d| {
                            if let Some(p) = d.product_mut(id) { p.nombre = event_target_value(&ev); }
                        })
                    />
                </td>
                <td class="py-1 pr-2 w-24">
                    <div class="flex items-center gap-1">
                        <input
                            type="text"
                            inputmode="decimal"
                            class=format!("{INPUT_CLASS} text-right")
                            aria-label="Cantidad"
                            prop:value=initial.cantidad.clone()
                            on:input=move |ev| draft.update(|d| {
                                if let Some(p) = d.product_mut(id) { p.cantidad = event_target_value(&ev); }
                            })
                        />
                        {unidad.map(|u| view! { <span class="text-xs text-gray-500">{u}</span> })}
                    </div>
                </td>
                <td class="py-1 pr-2 w-28">
                    <input
                        type="text"
                        inputmode="decimal"
                        class=format!("{INPUT_CLASS} text-right")
                        aria-label="Importe en euros"
                        prop:value=initial.importe.clone()
                        on:input=move |ev| draft.update(|d| {
                            if let Some(p) = d.product_mut(id) { p.importe = event_target_value(&ev); }
                        })
                    />
                </td>
                <td class="py-1 w-8 text-right">
                    <button
                        type="button"
                        class="mt-2 text-gray-400 hover:text-red-500 transition-colors"
                        title="Quitar producto"
                        on:click=move |_| draft.update(|d| d.productos.retain(|p| p.id != id))
                    >
                        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"></path>
                        </svg>
                    </button>
                </td>
            </tr>
        }
        .into_view()
    };

    // Comprobación en vivo: la suma de productos tiene que cuadrar con el total
    let sum_status = move || {
        draft.with(|d| match (d.products_sum(), parse_amount(&d.total)) {
            (Some(suma), Some(total)) if (suma - total).abs() < 0.005 => (
                "text-green-700 bg-green-50 border-green-100",
                format!(
                    "La suma de los productos cuadra con el total ({} €).",
                    euros(total)
                ),
            ),
            (Some(suma), Some(total)) => (
                "text-red-700 bg-red-50 border-red-100",
                format!(
                    "La suma de los productos es {} € y el total {} €: revisa los importes.",
                    euros(suma),
                    euros(total)
                ),
            ),
            _ => (
                "text-amber-700 bg-amber-50 border-amber-100",
                "Hay importes sin rellenar o que no son números.".to_string(),
            ),
        })
    };

    view! {
        <div class="space-y-4">
            <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
                {field("Nº de factura", "text", "numeric", "0000-000-000000",
                    |d| d.numero_factura.clone(), |d, v| d.numero_factura = v)}
                {field("Total (€)", "text", "decimal", "0,00",
                    |d| d.total.clone(), |d, v| d.total = v)}
                {field("Fecha", "date", "text", "",
                    |d| d.fecha.clone(), |d, v| d.fecha = v)}
                {field("Hora", "time", "text", "",
                    |d| d.hora.clone(), |d, v| d.hora = v)}
            </div>

            <div>
                <p class="text-xs font-semibold text-gray-600 uppercase tracking-wide mb-2">"Productos"</p>
                <div class="overflow-x-auto">
                    <table class="w-full text-sm">
                        <thead>
                            <tr class="text-left text-xs text-gray-500">
                                <th class="font-medium pb-1">"Nombre"</th>
                                <th class="font-medium pb-1">"Cant."</th>
                                <th class="font-medium pb-1">"Importe (€)"</th>
                                <th></th>
                            </tr>
                        </thead>
                        <tbody>
                            <For
                                each=move || draft.with(|d| d.productos.iter().map(|p| p.id).collect::<Vec<_>>())
                                key=|id| *id
                                children=product_row
                            />
                        </tbody>
                    </table>
                </div>
                <button
                    type="button"
                    class="mt-2 text-sm font-medium text-primary-600 hover:text-primary-700"
                    on:click=move |_| draft.update(TicketDraft::add_product)
                >
                    "+ Añadir producto"
                </button>
            </div>

            {move || {
                let (classes, message) = sum_status();
                view! { <p class=format!("text-sm rounded-lg border px-3 py-2 {classes}")>{message}</p> }
            }}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary() -> OcrResponseSummary {
        let producto =
            |nombre: &str, cantidad: f64, unidad: &str, pu: f64, total: f64| TicketProduct {
                nombre: nombre.into(),
                cantidad,
                unidad: unidad.into(),
                precio_unitario: pu,
                precio_total: total,
                descuento: 0.0,
                iva_porcentaje: 4.0,
                iva_importe: 0.06,
            };
        OcrResponseSummary {
            ticket_id: "t1".into(),
            numero_factura: Some("0000-000-000001".into()),
            fecha: Some("01/02/2026".into()),
            total: Some(3.49),
            productos_detectados: 2,
            productos: vec![
                producto("LECHE ENTERA", 2.0, "unidad", 0.95, 1.90),
                producto("BROCOLI", 0.59, "kg", 2.70, 1.59),
            ],
            tienda: None,
            processing_profile: Some("image-vision".into()),
            warnings: vec![],
            raw_text_preview: None,
            fecha_hora: Some("2026-02-01T18:30:00".into()),
            ubicacion: None,
            metodo_pago: Some("EFECTIVO".into()),
            numero_operacion: None,
            iva_desglose: vec![],
        }
    }

    #[test]
    fn prepara_el_borrador_con_formato_espanol() {
        let draft = TicketDraft::from_summary(&summary());
        assert_eq!(
            (draft.fecha.as_str(), draft.hora.as_str()),
            ("2026-02-01", "18:30")
        );
        assert_eq!(draft.total, "3,49");
        assert_eq!(draft.productos[1].cantidad, "0,59");
        assert_eq!(draft.productos[1].importe, "1,59");
    }

    #[test]
    fn sin_cambios_conserva_lo_leido_y_con_cambios_lo_recalcula() {
        let mut draft = TicketDraft::from_summary(&summary());
        let sin_cambios = draft.to_request().unwrap();
        assert_eq!(sin_cambios.fecha_hora, "2026-02-01T18:30:00");
        assert_eq!(sin_cambios.productos[1].iva_importe, 0.06);

        // El usuario corrige un dígito mal leído
        draft.productos[0].importe = "2,00".into();
        draft.total = "3,59".into();
        let corregido = draft.to_request().unwrap();
        assert_eq!(corregido.productos[0].precio_unitario, 1.0);
        assert_eq!(corregido.productos[0].iva_importe, 0.0);
        assert_eq!(corregido.productos[1].iva_importe, 0.06);
    }

    #[test]
    fn no_deja_guardar_un_ticket_que_no_cuadra_o_incompleto() {
        let mut draft = TicketDraft::from_summary(&summary());
        draft.total = "9,99".into();
        assert!(draft.to_request().unwrap_err().contains("no coincide"));

        let mut draft = TicketDraft::from_summary(&summary());
        draft.numero_factura = "0000-00O-000001".into();
        assert!(draft.to_request().unwrap_err().contains("formato"));

        let mut draft = TicketDraft::from_summary(&summary());
        draft.hora.clear();
        assert!(draft.to_request().is_err());
    }

    #[test]
    fn acepta_importes_con_coma_o_punto() {
        assert_eq!(parse_amount("1,59"), Some(1.59));
        assert_eq!(parse_amount(" 1.59 € "), Some(1.59));
        assert_eq!(parse_amount("abc"), None);
    }
}
