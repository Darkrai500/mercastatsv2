//! Piezas de la integración con Google que comparten las rutas y la sincronización.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use crate::config::GoogleConfig;

use super::crypto::TokenCipher;
use super::gmail::GmailClient;
use super::google::GoogleOAuth;

pub struct GoogleIntegration {
    pub oauth: GoogleOAuth,
    pub gmail: GmailClient,
    pub cipher: TokenCipher,
    /// Dirección de la que se importan los tickets (en minúsculas)
    pub sender: String,
    /// La cookie del estado OAuth lleva `Secure` solo si el redirect es HTTPS
    pub secure_cookies: bool,
    running: Mutex<HashSet<String>>,
}

impl GoogleIntegration {
    pub fn new(config: &GoogleConfig, sender: &str) -> Result<Self, reqwest::Error> {
        Ok(Self {
            oauth: GoogleOAuth::new(config)?,
            gmail: GmailClient::new()?,
            cipher: TokenCipher::new(&config.token_key),
            sender: sender.to_ascii_lowercase(),
            secure_cookies: config.is_https(),
            running: Mutex::new(HashSet::new()),
        })
    }

    /// Apunta Google y Gmail a un servidor simulado (solo tests).
    #[cfg(test)]
    pub fn with_fake_google(self, base: &str) -> Self {
        Self {
            oauth: self.oauth.with_base_url(base),
            gmail: self.gmail.with_base_url(base),
            ..self
        }
    }

    /// Búsqueda de Gmail que localiza los correos con ticket.
    pub fn search_query(&self) -> String {
        format!("from:{} has:attachment filename:pdf", self.sender)
    }

    /// Reserva la sincronización de un usuario; `None` si ya hay una en curso.
    pub fn try_begin_sync(self: &Arc<Self>, email: &str) -> Option<SyncPermit> {
        let mut running = self.running.lock().unwrap_or_else(|e| e.into_inner());
        if !running.insert(email.to_string()) {
            return None;
        }
        Some(SyncPermit {
            integration: Arc::clone(self),
            email: email.to_string(),
        })
    }

    pub fn is_syncing(&self, email: &str) -> bool {
        self.running
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(email)
    }
}

/// Mientras exista, el usuario figura como "sincronizando". Se libera al soltarlo, también si
/// la tarea falla o se cancela.
pub struct SyncPermit {
    integration: Arc<GoogleIntegration>,
    email: String,
}

impl Drop for SyncPermit {
    fn drop(&mut self) {
        self.integration
            .running
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.email);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn integration() -> Arc<GoogleIntegration> {
        let cfg = GoogleConfig {
            client_id: "id".into(),
            client_secret: "secret".into(),
            redirect_uri: "http://localhost:3000/api/auth/google/callback".into(),
            token_key: [1; 32],
        };
        Arc::new(GoogleIntegration::new(&cfg, "Ticket_Digital@Mail.Mercadona.com").unwrap())
    }

    #[test]
    fn la_busqueda_usa_el_remitente_en_minusculas() {
        assert_eq!(
            integration().search_query(),
            "from:ticket_digital@mail.mercadona.com has:attachment filename:pdf"
        );
    }

    #[test]
    fn no_permite_dos_sincronizaciones_del_mismo_usuario_a_la_vez() {
        let g = integration();
        let permiso = g
            .try_begin_sync("ana@example.com")
            .expect("primera reserva");
        assert!(g.is_syncing("ana@example.com"));
        assert!(g.try_begin_sync("ana@example.com").is_none());
        // Otro usuario no se ve afectado
        assert!(g.try_begin_sync("luis@example.com").is_some());
        drop(permiso);
        assert!(!g.is_syncing("ana@example.com"));
        assert!(g.try_begin_sync("ana@example.com").is_some());
    }
}
