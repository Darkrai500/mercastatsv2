use std::path::PathBuf;

use pyo3::{
    exceptions::PyException,
    prelude::*,
    types::{PyList, PyModule},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Request para procesamiento OCR desde Rust.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessTicketRequest {
    pub ticket_id: String,
    pub file_name: String,
    pub pdf_b64: String,
}

/// Producto detectado en el ticket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketProduct {
    pub nombre: String,
    pub cantidad: f64,
    pub unidad: String,
    pub precio_unitario: f64,
    pub precio_total: f64,
    #[serde(default)]
    pub descuento: f64,
    #[serde(default)]
    pub iva_porcentaje: f64,
    #[serde(default)]
    pub iva_importe: f64,
}

/// Desglose de IVA reportado en el ticket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IvaBreakdown {
    pub porcentaje: f64,
    pub base_imponible: f64,
    pub cuota: f64,
}

/// Respuesta completa del procesamiento.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessTicketResponse {
    pub ticket_id: String,
    pub raw_text: String,
    pub numero_factura: Option<String>,
    pub fecha: Option<String>,
    pub fecha_hora: Option<String>,
    pub total: Option<f64>,
    pub tienda: Option<String>,
    pub ubicacion: Option<String>,
    pub metodo_pago: Option<String>,
    pub numero_operacion: Option<String>,
    #[serde(default)]
    pub productos: Vec<TicketProduct>,
    #[serde(default)]
    pub iva_desglose: Vec<IvaBreakdown>,
}

/// Errores posibles al interactuar con el motor OCR.
#[derive(Debug, Error)]
pub enum OcrError {
    #[error("No se pudo procesar el ticket: {0}")]
    Parsing(String),
    #[error("Error en la integración Python: {0}")]
    Python(String),
    #[error(transparent)]
    Deserialize(#[from] serde_json::Error),
    #[error(transparent)]
    Join(#[from] tokio::task::JoinError),
}

/// Inicializa el entorno de Python y fuerza la carga de módulos pesados.
/// Debe ejecutarse al inicio del programa (Warm-up).
///
/// Esta función configura el `sys.path` y carga el módulo procesador de tickets,
/// lo que provoca que Python importe todas las dependencias pesadas
/// (pdfplumber, pdfminer.six, pydantic) en tiempo de arranque en lugar de
/// en la primera petición.
///
/// # Errors
///
/// Retorna `OcrError::Python` si no se puede cargar el módulo o configurar Python.
pub fn init_python_worker() -> Result<(), OcrError> {
    tracing::info!("🐍 Warm-up: Inicializando intérprete de Python y cargando dependencias...");

    Python::with_gil(|py| {
        // Forzamos la carga del módulo procesador.
        // Al hacer esto, Python ejecuta todos los imports de nivel superior 
        // en 'processor.py' y 'pdf_parser.py' (incluyendo pdfplumber).
        load_processor_module(py)?;
        
        tracing::info!("✅ Warm-up: Módulos Python cargados y listos en memoria.");
        Ok(())
    })
}

/// Procesa un ticket PDF utilizando la lógica Python embebida.
pub async fn process_ticket(
    request: ProcessTicketRequest,
) -> Result<ProcessTicketResponse, OcrError> {
    let request_clone = request.clone();

    let json_payload = tokio::task::spawn_blocking(move || {
        Python::with_gil(|py| process_ticket_internal(py, &request_clone))
    })
    .await??;

    let response: ProcessTicketResponse = serde_json::from_str(&json_payload)?;
    Ok(response)
}

fn process_ticket_internal(
    py: Python<'_>,
    request: &ProcessTicketRequest,
) -> Result<String, OcrError> {
    let module = load_processor_module(py)?;
    let func = module.getattr("process_ticket_json").map_err(|err| {
        OcrError::Python(format!(
            "Función process_ticket_json no encontrada: {}",
            err
        ))
    })?;

    let args = (
        request.ticket_id.as_str(),
        request.file_name.as_str(),
        request.pdf_b64.as_str(),
    );

    match func.call1(args) {
        Ok(py_obj) => py_obj
            .extract::<String>()
            .map_err(|err| OcrError::Python(format!("Respuesta inesperada: {}", err))),
        Err(err) => handle_python_error(py, err),
    }
}

#[allow(deprecated)]
fn load_processor_module(py: Python<'_>) -> Result<&PyModule, OcrError> {
    let sys = py
        .import("sys")
        .map_err(|err| OcrError::Python(format!("No se pudo importar sys: {}", err)))?;

    let path_obj = sys
        .getattr("path")
        .map_err(|err| OcrError::Python(format!("No se pudo acceder a sys.path: {}", err)))?;

    let path: &PyList = path_obj
        .downcast::<PyList>()
        .map_err(|err| OcrError::Python(format!("No se pudo interpretar sys.path: {}", err)))?;

    let module_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("ocr-service");
    let module_path_str = module_path
        .to_str()
        .ok_or_else(|| OcrError::Python("Ruta de módulo inválida".to_string()))?;

    let already_present = path.iter().any(|entry| match entry.extract::<&str>() {
        Ok(value) => value == module_path_str,
        Err(_) => false,
    });

    if !already_present {
        path.insert(0, module_path_str).map_err(|err| {
            OcrError::Python(format!(
                "No se pudo añadir {} a sys.path: {}",
                module_path_str, err
            ))
        })?;
    }

    // El paquete Python es `src`, importamos `src.processor` para mantener los imports relativos
    py.import("src.processor")
        .map_err(|err| OcrError::Python(format!("No se pudo importar processor: {}", err)))
}

#[allow(deprecated)]
fn handle_python_error(py: Python<'_>, err: PyErr) -> Result<String, OcrError> {
    let parsing_error = py
        .import("src.processor")
        .ok()
        .and_then(|module| module.getattr("PDFParsingError").ok());

    if let Some(parsing_error) = parsing_error {
        if err.is_instance(py, parsing_error) {
            return Err(OcrError::Parsing(err.to_string()));
        }
    }

    if err.is_instance_of::<PyException>(py) {
        Err(OcrError::Python(err.to_string()))
    } else {
        Err(OcrError::Python(format!("Error Python: {}", err)))
    }
}
