use crate::{
    db,
    error::{AppError, AppResult},
    models::{ProductUpsert, PurchaseInsert, PurchaseProductInsert, TicketPdfInsert},
    services::{OcrProcessTicketResponse as ProcessTicketResponse, TicketProduct},
};
use base64::{engine::general_purpose, Engine as _};
use chrono::{Local, NaiveDateTime, TimeZone, Utc};
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::str::FromStr;

/// Respuesta de la ingesta de un ticket
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketIngestionResponse {
    pub ingested: bool,
    pub numero_factura: String,
    pub total: Decimal,
    pub productos_insertados: usize,
    pub fecha_hora: NaiveDateTime,
}

/// Procesa e ingesta un ticket completo en la base de datos
///
/// Esta función orquesta todo el proceso:
/// 1. Valida los datos obligatorios del ticket
/// 2. Normaliza los datos
/// 3. Verifica que no exista duplicado
/// 4. Decodifica el PDF
/// 5. Abre una transacción y ejecuta:
///    - Upsert de productos
///    - Insert de la compra
///    - Insert de compras_productos
///    - Insert del PDF
/// 6. Retorna resumen de la operación
pub async fn ingest_ticket(
    pool: &PgPool,
    user_email: &str,
    pdf_b64: &str,
    file_name: &str,
    ocr_response: ProcessTicketResponse,
) -> AppResult<TicketIngestionResponse> {
    // 1. Validar campos obligatorios
    let numero_factura = ocr_response
        .numero_factura
        .as_ref()
        .ok_or(AppError::MissingInvoiceNumber)?;

    let numero_factura = PurchaseInsert::normalize_invoice_number(numero_factura);

    tracing::info!("🔄 Iniciando ingesta de ticket: {}", numero_factura);

    // 2. Verificar si ya existe (idempotencia)
    if let Some(_existing) = db::get_purchase(pool, &numero_factura).await? {
        tracing::warn!(
            "⚠️  La compra {} ya existe en la base de datos",
            numero_factura
        );
        return Err(AppError::DuplicatePurchase(numero_factura));
    }

    // 3. Extraer y validar datos principales
    let fecha_hora = parse_fecha_hora(&ocr_response)?;
    let total = parse_total(&ocr_response)?;

    // 4. Validar que haya productos
    if ocr_response.productos.is_empty() && total > Decimal::ZERO {
        return Err(AppError::InvalidTicketData(
            "El ticket tiene un total mayor a 0 pero no contiene productos".to_string(),
        ));
    }

    // 5. Decodificar el PDF
    let pdf_bytes = decode_pdf_base64(pdf_b64)?;

    // Validar el PDF
    let ticket_pdf = TicketPdfInsert {
        numero_factura: numero_factura.clone(),
        ticket_pdf: pdf_bytes,
        ticket_nombre_archivo: file_name.to_string(),
    };
    ticket_pdf.validate().map_err(AppError::InvalidTicketData)?;

    // 6. Preparar datos de la compra
    let purchase_data = PurchaseInsert {
        numero_factura: numero_factura.clone(),
        usuario_email: user_email.to_string(),
        fecha_hora,
        total,
        tienda: ocr_response.tienda.map(|t| t.trim().to_string()),
        ubicacion: ocr_response.ubicacion.map(|u| u.trim().to_string()),
        metodo_pago: ocr_response
            .metodo_pago
            .and_then(|m| PurchaseInsert::normalize_payment_method(&m)),
        numero_operacion: ocr_response.numero_operacion.map(|n| n.trim().to_string()),
    };

    // 7. Preparar productos
    let productos: Vec<PurchaseProductInsert> = ocr_response
        .productos
        .iter()
        .map(|p| parse_producto(p))
        .collect::<AppResult<Vec<_>>>()?;

    // 8. Validar coherencia de totales
    validate_totals(&productos, total)?;

    // 9. Ejecutar inserción en transacción
    let mut tx = pool.begin().await?;

    tracing::debug!("📦 Procesando {} productos...", productos.len());

    // Upsert de productos en el catálogo
    for producto in &productos {
        let product_upsert = ProductUpsert {
            nombre: producto.producto_nombre.clone(),
            marca: None,                  // El OCR actual no extrae marca
            unidad: "unidad".to_string(), // TODO: extraer del OCR
            precio_actual: Some(producto.precio_unitario),
        };

        db::upsert_product(&mut *tx, &product_upsert).await?;
    }

    // Insertar compra
    tracing::debug!("🛒 Insertando compra: {}", numero_factura);
    let _purchase = db::insert_purchase(&mut *tx, &purchase_data).await?;

    // Insertar productos de la compra
    tracing::debug!(
        "📋 Insertando {} productos de la compra...",
        productos.len()
    );

    // Insertar productos uno por uno dentro de la transacción
    let mut rows_inserted = 0u64;
    for producto in &productos {
        let result = sqlx::query!(
            r#"
            INSERT INTO compras_productos (
                compra_numero_factura,
                producto_nombre,
                cantidad,
                precio_unitario,
                precio_total,
                descuento,
                iva_porcentaje,
                iva_importe
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
            numero_factura,
            producto.producto_nombre,
            producto.cantidad,
            producto.precio_unitario,
            producto.precio_total,
            producto.descuento,
            producto.iva_porcentaje,
            producto.iva_importe
        )
        .execute(&mut *tx)
        .await?;

        rows_inserted += result.rows_affected();
    }

    // Insertar PDF del ticket
    tracing::debug!(
        "📄 Guardando PDF ({} bytes)...",
        ticket_pdf.ticket_pdf.len()
    );
    db::insert_ticket_pdf(&mut *tx, &ticket_pdf).await?;

    // Commit de la transacción
    tx.commit().await?;

    tracing::info!(
        "✅ Ticket {} ingestado exitosamente ({} productos)",
        numero_factura,
        rows_inserted
    );

    Ok(TicketIngestionResponse {
        ingested: true,
        numero_factura,
        total,
        productos_insertados: rows_inserted as usize,
        fecha_hora,
    })
}

/// Parsea la fecha y hora del ticket
fn parse_fecha_hora(response: &ProcessTicketResponse) -> AppResult<NaiveDateTime> {
    // Convierte un NaiveDateTime (interpretado como hora local) a UTC para guardar en DB
    fn to_utc_naive(dt: NaiveDateTime) -> NaiveDateTime {
        Local
            .from_local_datetime(&dt)
            .single()
            .or_else(|| Local.from_local_datetime(&dt).earliest())
            .map(|dt_local| dt_local.with_timezone(&Utc).naive_utc())
            .unwrap_or(dt)
    }

    // Intentar con fecha_hora primero
    if let Some(ref fecha_hora_str) = response.fecha_hora {
        if let Ok(dt) = NaiveDateTime::parse_from_str(fecha_hora_str, "%Y-%m-%d %H:%M:%S") {
            return Ok(to_utc_naive(dt));
        }
        // Intentar con formato ISO
        if let Ok(dt) = NaiveDateTime::parse_from_str(fecha_hora_str, "%Y-%m-%dT%H:%M:%S") {
            return Ok(to_utc_naive(dt));
        }
    }

    // Fallback a fecha sola con hora por defecto
    if let Some(ref fecha_str) = response.fecha {
        if let Ok(date) = chrono::NaiveDate::parse_from_str(fecha_str, "%Y-%m-%d") {
            tracing::warn!(
                "⚠️  Fecha sin hora detectada, usando 12:00:00 por defecto para ticket {}",
                response.ticket_id
            );
            let dt = date.and_hms_opt(12, 0, 0).unwrap();
            return Ok(to_utc_naive(dt));
        }
    }

    Err(AppError::InvalidTicketData(
        "No se pudo parsear la fecha del ticket".to_string(),
    ))
}

/// Parsea el total del ticket
fn parse_total(response: &ProcessTicketResponse) -> AppResult<Decimal> {
    let total_f64 = response
        .total
        .ok_or_else(|| AppError::InvalidTicketData("El ticket no tiene total".to_string()))?;

    if total_f64 < 0.0 {
        return Err(AppError::InvalidTotals(
            "El total no puede ser negativo".to_string(),
        ));
    }

    // Convertir f64 a Decimal con precisión
    Decimal::from_str(&format!("{:.2}", total_f64))
        .map_err(|_| AppError::InvalidTicketData(format!("Total inválido: {}", total_f64)))
}

/// Parsea un producto del ticket OCR a PurchaseProductInsert
fn parse_producto(producto: &TicketProduct) -> AppResult<PurchaseProductInsert> {
    let nombre = ProductUpsert::normalize_name(&producto.nombre);

    if nombre.is_empty() {
        return Err(AppError::InvalidTicketData(
            "Producto con nombre vacío".to_string(),
        ));
    }

    let cantidad = Decimal::from_f64(producto.cantidad).ok_or_else(|| {
        AppError::InvalidTicketData(format!("Cantidad inválida: {}", producto.cantidad))
    })?;

    let precio_unitario = Decimal::from_f64(producto.precio_unitario).ok_or_else(|| {
        AppError::InvalidTicketData(format!(
            "Precio unitario inválido: {}",
            producto.precio_unitario
        ))
    })?;

    let precio_total = Decimal::from_f64(producto.precio_total).ok_or_else(|| {
        AppError::InvalidTicketData(format!("Precio total inválido: {}", producto.precio_total))
    })?;

    let descuento = Decimal::from_f64(producto.descuento).unwrap_or(Decimal::ZERO);

    let iva_porcentaje = Decimal::from_f64(producto.iva_porcentaje).unwrap_or(Decimal::ZERO);
    let iva_porcentaje = PurchaseProductInsert::normalize_iva_percentage(iva_porcentaje);

    let iva_importe = if producto.iva_importe > 0.0 {
        Decimal::from_f64(producto.iva_importe).unwrap_or(Decimal::ZERO)
    } else {
        // Calcular IVA si no está presente
        let base = precio_total - descuento;
        PurchaseProductInsert::calculate_iva_importe(base, iva_porcentaje)
    };

    let product_insert = PurchaseProductInsert {
        producto_nombre: nombre,
        cantidad,
        precio_unitario,
        precio_total,
        descuento,
        iva_porcentaje,
        iva_importe,
    };

    // Validar coherencia de precios
    if !product_insert.validate_price_coherence() {
        tracing::warn!(
            "⚠️  Incoherencia de precios en producto {}: {} × {:.2} - {:.2} ≠ {:.2}",
            product_insert.producto_nombre,
            product_insert.cantidad,
            product_insert.precio_unitario,
            product_insert.descuento,
            product_insert.precio_total
        );
    }

    Ok(product_insert)
}

/// Valida que la suma de productos coincida con el total del ticket
fn validate_totals(productos: &[PurchaseProductInsert], expected_total: Decimal) -> AppResult<()> {
    let suma_productos: Decimal = productos.iter().map(|p| p.precio_total).sum();

    let diff = (suma_productos - expected_total).abs();
    let tolerance = Decimal::new(10, 2); // 0.10€ de tolerancia

    if diff > tolerance {
        return Err(AppError::InvalidTotals(format!(
            "La suma de productos ({:.2}) no coincide con el total del ticket ({:.2}). Diferencia: {:.2}",
            suma_productos, expected_total, diff
        )));
    }

    if diff > Decimal::ZERO {
        tracing::warn!(
            "⚠️  Pequeña diferencia en totales: {:.2} (dentro de tolerancia)",
            diff
        );
    }

    Ok(())
}

/// Decodifica el PDF en base64 a bytes
fn decode_pdf_base64(pdf_b64: &str) -> AppResult<Vec<u8>> {
    general_purpose::STANDARD
        .decode(pdf_b64)
        .map_err(|e| AppError::InvalidTicketData(format!("PDF base64 inválido: {}", e)))
}
