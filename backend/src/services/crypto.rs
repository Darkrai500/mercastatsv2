//! Primitivas criptográficas de la integración con Google, sobre `ring`.
//!
//! - Cifrado del refresh token en reposo (AES-256-GCM).
//! - Aleatorios para `state` y para el verificador PKCE, y el desafío S256.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM, NONCE_LEN};
use ring::digest::{digest, SHA256};
use ring::hmac;
use ring::rand::{SecureRandom, SystemRandom};

/// Error criptográfico. A propósito sin detalle: nada de lo cifrado debe acabar en un log.
#[derive(Debug, PartialEq, Eq)]
pub struct CryptoError;

impl std::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "error criptográfico")
    }
}

impl std::error::Error for CryptoError {}

/// Cifra y descifra secretos con una clave AES-256 fija.
///
/// Formato: `nonce (12 bytes) || ciphertext || tag (16 bytes)`. El nonce es aleatorio por
/// cifrado. Los "datos asociados" (AAD) quedan autenticados pero no cifrados: se usa el email
/// del dueño para que un token copiado a la fila de otro usuario no se pueda descifrar.
pub struct TokenCipher {
    key: LessSafeKey,
}

impl TokenCipher {
    pub fn new(key_bytes: &[u8; 32]) -> Self {
        // Solo falla si la longitud no es la del algoritmo, y el tipo ya la garantiza.
        let unbound = UnboundKey::new(&AES_256_GCM, key_bytes).expect("clave AES-256 de 32 bytes");
        Self {
            key: LessSafeKey::new(unbound),
        }
    }

    pub fn encrypt(&self, plaintext: &[u8], aad: &str) -> Result<Vec<u8>, CryptoError> {
        let mut nonce_bytes = [0u8; NONCE_LEN];
        SystemRandom::new()
            .fill(&mut nonce_bytes)
            .map_err(|_| CryptoError)?;

        let mut in_out = plaintext.to_vec();
        self.key
            .seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce_bytes),
                Aad::from(aad.as_bytes()),
                &mut in_out,
            )
            .map_err(|_| CryptoError)?;

        let mut out = Vec::with_capacity(NONCE_LEN + in_out.len());
        out.extend_from_slice(&nonce_bytes);
        out.extend_from_slice(&in_out);
        Ok(out)
    }

    pub fn decrypt(&self, data: &[u8], aad: &str) -> Result<Vec<u8>, CryptoError> {
        if data.len() < NONCE_LEN + AES_256_GCM.tag_len() {
            return Err(CryptoError);
        }
        let (nonce_bytes, ciphertext) = data.split_at(NONCE_LEN);
        let nonce = Nonce::try_assume_unique_for_key(nonce_bytes).map_err(|_| CryptoError)?;

        let mut buffer = ciphertext.to_vec();
        let plaintext = self
            .key
            .open_in_place(nonce, Aad::from(aad.as_bytes()), &mut buffer)
            .map_err(|_| CryptoError)?;
        Ok(plaintext.to_vec())
    }
}

/// `n` bytes aleatorios del sistema, codificados en base64 URL-safe sin relleno.
pub fn random_urlsafe(n: usize) -> Result<String, CryptoError> {
    let mut bytes = vec![0u8; n];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| CryptoError)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

/// Desafío PKCE método S256: `BASE64URL(SHA256(verifier))` (RFC 7636, apartado 4.2).
pub fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(digest(&SHA256, verifier.as_bytes()))
}

/// Comparación en tiempo constante (para el `state` del callback).
///
/// `ring` ya no expone una comparación directa; la forma recomendada es comparar las etiquetas
/// HMAC de ambos valores con una clave aleatoria de usar y tirar (`hmac::verify` es de tiempo
/// constante y, de paso, no revela si las longitudes coinciden).
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let Ok(key) = hmac::Key::generate(hmac::HMAC_SHA256, &SystemRandom::new()) else {
        return false;
    };
    let tag = hmac::sign(&key, a.as_bytes());
    hmac::verify(&key, b.as_bytes(), tag.as_ref()).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cipher() -> TokenCipher {
        TokenCipher::new(&[42u8; 32])
    }

    #[test]
    fn cifra_y_descifra() {
        let c = cipher();
        let cifrado = c
            .encrypt(b"1//refresh-token-sintetico", "ana@example.com")
            .unwrap();
        assert_eq!(
            c.decrypt(&cifrado, "ana@example.com").unwrap(),
            b"1//refresh-token-sintetico"
        );
    }

    #[test]
    fn el_texto_cifrado_no_contiene_el_original() {
        let cifrado = cipher()
            .encrypt(b"1//refresh-token-sintetico", "a@example.com")
            .unwrap();
        let hay = |needle: &[u8]| cifrado.windows(needle.len()).any(|w| w == needle);
        assert!(!hay(b"refresh-token"));
    }

    #[test]
    fn dos_cifrados_del_mismo_texto_difieren() {
        let c = cipher();
        let a = c.encrypt(b"mismo", "a@example.com").unwrap();
        let b = c.encrypt(b"mismo", "a@example.com").unwrap();
        assert_ne!(a, b, "el nonce debe ser distinto en cada cifrado");
    }

    #[test]
    fn rechaza_una_clave_distinta() {
        let cifrado = cipher().encrypt(b"secreto", "a@example.com").unwrap();
        let otra = TokenCipher::new(&[43u8; 32]);
        assert_eq!(otra.decrypt(&cifrado, "a@example.com"), Err(CryptoError));
    }

    #[test]
    fn rechaza_el_token_copiado_a_otro_usuario() {
        let c = cipher();
        let cifrado = c.encrypt(b"secreto", "ana@example.com").unwrap();
        assert_eq!(c.decrypt(&cifrado, "otro@example.com"), Err(CryptoError));
    }

    #[test]
    fn detecta_manipulacion() {
        let c = cipher();
        let mut cifrado = c.encrypt(b"secreto", "a@example.com").unwrap();
        let ultimo = cifrado.len() - 1;
        cifrado[ultimo] ^= 1;
        assert_eq!(c.decrypt(&cifrado, "a@example.com"), Err(CryptoError));
    }

    #[test]
    fn rechaza_datos_demasiado_cortos() {
        assert_eq!(
            cipher().decrypt(&[1, 2, 3], "a@example.com"),
            Err(CryptoError)
        );
        assert_eq!(cipher().decrypt(&[], "a@example.com"), Err(CryptoError));
    }

    #[test]
    fn pkce_cumple_el_vector_del_rfc_7636() {
        // RFC 7636, Apéndice B
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn los_aleatorios_son_urlsafe_y_distintos() {
        let a = random_urlsafe(32).unwrap();
        let b = random_urlsafe(32).unwrap();
        assert_ne!(a, b);
        assert_eq!(a.len(), 43); // 32 bytes -> 43 caracteres base64url sin relleno
        assert!(a
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn comparacion_en_tiempo_constante() {
        assert!(constant_time_eq("abc", "abc"));
        assert!(!constant_time_eq("abc", "abd"));
        assert!(!constant_time_eq("abc", "abcd"));
    }
}
