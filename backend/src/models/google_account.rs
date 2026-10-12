use chrono::{DateTime, Utc};

pub const ESTADO_ACTIVA: &str = "activa";
pub const ESTADO_REQUIERE_RECONEXION: &str = "requiere_reconexion";

/// Fila de `cuentas_google`. Sin `Debug` ni `Serialize` a propósito: lleva el refresh token
/// (cifrado) y no debe poder acabar en un log ni en una respuesta.
#[derive(Clone, sqlx::FromRow)]
pub struct GoogleAccount {
    pub usuario_email: String,
    pub google_email: String,
    pub refresh_token_cifrado: Vec<u8>,
    pub estado: String,
    pub ultima_sync_fin: Option<DateTime<Utc>>,
    pub ultima_sync_ok: Option<bool>,
    pub ultima_sync_error: Option<String>,
    pub ultima_sync_importados: i32,
    pub ultima_sync_duplicados: i32,
    pub ultima_sync_fallidos: i32,
}

/// Resultado del tratamiento de un mensaje de Gmail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageOutcome {
    Importado,
    Duplicado,
    SinPdf,
    Fallido,
}

impl MessageOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Importado => "importado",
            Self::Duplicado => "duplicado",
            Self::SinPdf => "sin_pdf",
            Self::Fallido => "fallido",
        }
    }
}

/// Contadores de una ejecución de la sincronización.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SyncCounts {
    pub importados: u32,
    pub duplicados: u32,
    pub fallidos: u32,
}
