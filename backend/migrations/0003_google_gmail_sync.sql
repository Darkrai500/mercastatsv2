-- =========================================================================
-- MERCASTATS - Inicio de sesión con Google y sincronización de tickets desde Gmail
-- =========================================================================
-- IDEMPOTENTE a propósito: docker-compose solo ejecuta los scripts de
-- docker-entrypoint-initdb.d con el volumen vacío, así que ./run.sh vuelve a
-- aplicar esta migración sobre las bases de datos ya existentes en cada arranque.
-- Cualquier migración posterior a la 0002 debe poder ejecutarse varias veces.
-- =========================================================================

-- 1. Cuentas creadas con Google no tienen contraseña.
ALTER TABLE usuarios ALTER COLUMN password_hash DROP NOT NULL;
ALTER TABLE usuarios DROP CONSTRAINT IF EXISTS password_hash_length;
ALTER TABLE usuarios ADD CONSTRAINT password_hash_length
    CHECK (password_hash IS NULL OR length(password_hash) >= 60); -- bcrypt

-- 2. Cuenta de Google vinculada a un usuario (como máximo una por usuario).
--    El refresh token se guarda CIFRADO (AES-256-GCM, nonce || ciphertext || tag);
--    la clave vive solo en la variable de entorno TOKEN_ENCRYPTION_KEY.
CREATE TABLE IF NOT EXISTS cuentas_google (
    usuario_email VARCHAR(255) PRIMARY KEY,
    google_sub VARCHAR(255) NOT NULL,
    google_email VARCHAR(255) NOT NULL,
    refresh_token_cifrado BYTEA NOT NULL,
    -- 'activa' | 'requiere_reconexion' (token revocado o caducado: no se sincroniza)
    estado VARCHAR(30) NOT NULL DEFAULT 'activa',
    conectado_en TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    ultima_sync_inicio TIMESTAMPTZ,
    ultima_sync_fin TIMESTAMPTZ,
    ultima_sync_ok BOOLEAN,
    -- Código corto (p. ej. 'token_revocado'); nunca texto con datos del usuario.
    ultima_sync_error VARCHAR(60),
    ultima_sync_importados INTEGER NOT NULL DEFAULT 0,
    ultima_sync_duplicados INTEGER NOT NULL DEFAULT 0,
    ultima_sync_fallidos INTEGER NOT NULL DEFAULT 0,

    CONSTRAINT fk_cuentas_google_usuario
        FOREIGN KEY (usuario_email)
        REFERENCES usuarios(email)
        ON DELETE CASCADE
        ON UPDATE CASCADE,
    CONSTRAINT unique_cuentas_google_sub UNIQUE (google_sub),
    CONSTRAINT check_cuentas_google_estado CHECK (estado IN ('activa', 'requiere_reconexion'))
);

COMMENT ON TABLE cuentas_google IS 'Cuenta de Google vinculada a un usuario, con el refresh token cifrado para leer Gmail';
COMMENT ON COLUMN cuentas_google.refresh_token_cifrado IS 'AES-256-GCM: nonce(12) || ciphertext || tag(16)';

-- 3. Correos de Gmail ya tratados, para no volver a descargar ni a pasar por OCR.
--    Los fallos transitorios (OCR caído, red) NO se registran: se reintentan en la
--    siguiente sincronización. Los fallos de contenido se reintentan hasta 3 veces.
CREATE TABLE IF NOT EXISTS gmail_mensajes_procesados (
    usuario_email VARCHAR(255) NOT NULL,
    gmail_message_id VARCHAR(64) NOT NULL,
    resultado VARCHAR(20) NOT NULL,
    intentos INTEGER NOT NULL DEFAULT 1,
    procesado_en TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,

    PRIMARY KEY (usuario_email, gmail_message_id),
    CONSTRAINT fk_gmail_mensajes_usuario
        FOREIGN KEY (usuario_email)
        REFERENCES usuarios(email)
        ON DELETE CASCADE
        ON UPDATE CASCADE,
    CONSTRAINT check_gmail_mensajes_resultado
        CHECK (resultado IN ('importado', 'duplicado', 'sin_pdf', 'fallido'))
);

COMMENT ON TABLE gmail_mensajes_procesados IS 'Mensajes de Gmail ya tratados por la sincronización automática';
