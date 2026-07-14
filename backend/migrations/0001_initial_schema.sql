-- =========================================================================
-- MERCASTATS - Schema de Base de Datos PostgreSQL
-- =========================================================================
-- DescripciÃ³n: Script de creaciÃ³n completa de la base de datos para el MVP
-- VersiÃ³n: 1.0
-- Fecha: 2025-01-24
-- Autor: Juan Carlos
-- =========================================================================

-- =========================================================================
-- 1. EXTENSIONES DE POSTGRESQL
-- =========================================================================

-- UUID para generar identificadores Ãºnicos si los necesitamos en el futuro
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- pg_trgm para bÃºsqueda de texto eficiente (fuzzy search de productos)
CREATE EXTENSION IF NOT EXISTS "pg_trgm";

-- EstadÃ­sticas de queries para optimizaciÃ³n
CREATE EXTENSION IF NOT EXISTS "pg_stat_statements";

-- =========================================================================
-- 2. ELIMINACIÃ“N DE OBJETOS EXISTENTES (para re-ejecuciÃ³n del script)
-- =========================================================================

-- Drop tables en orden inverso por dependencias
DROP TABLE IF EXISTS preferencias_usuario CASCADE;
DROP TABLE IF EXISTS logros_usuario CASCADE;
DROP TABLE IF EXISTS logros CASCADE;
DROP TABLE IF EXISTS objetivos_ahorro CASCADE;
DROP TABLE IF EXISTS compras_productos CASCADE;
DROP TABLE IF EXISTS tickets_pdf CASCADE;
DROP TABLE IF EXISTS compras CASCADE;
DROP TABLE IF EXISTS historico_precios CASCADE;
DROP TABLE IF EXISTS productos CASCADE;
DROP TABLE IF EXISTS usuarios CASCADE;

-- Drop functions y triggers
DROP FUNCTION IF EXISTS actualizar_updated_at() CASCADE;
DROP FUNCTION IF EXISTS actualizar_precio_producto() CASCADE;
DROP FUNCTION IF EXISTS registrar_precio_historico() CASCADE;

-- =========================================================================
-- 3. TABLA: USUARIOS
-- =========================================================================
-- Almacena la informaciÃ³n de los usuarios de la aplicaciÃ³n
-- PK: email (clave natural)
-- =========================================================================

CREATE TABLE usuarios (
    email VARCHAR(255) PRIMARY KEY,
    password_hash VARCHAR(255) NOT NULL,
    nombre VARCHAR(255),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    
    -- Constraints
    CONSTRAINT email_format CHECK (email ~* '^[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}$'),
    CONSTRAINT password_hash_length CHECK (length(password_hash) >= 60) -- bcrypt hash
);

-- Ãndices
CREATE INDEX idx_usuarios_nombre ON usuarios(nombre);
CREATE INDEX idx_usuarios_created_at ON usuarios(created_at DESC);

-- Comentarios
COMMENT ON TABLE usuarios IS 'Usuarios registrados en la aplicaciÃ³n';
COMMENT ON COLUMN usuarios.email IS 'Email del usuario (clave primaria natural)';
COMMENT ON COLUMN usuarios.password_hash IS 'Hash bcrypt del password (nunca almacenar en texto plano)';
COMMENT ON COLUMN usuarios.updated_at IS 'Fecha de Ãºltima actualizaciÃ³n del registro';

-- =========================================================================
-- 4. TABLA: PRODUCTOS
-- =========================================================================
-- CatÃ¡logo de productos Ãºnicos identificados por su nombre
-- PK: nombre (clave natural normalizada)
-- =========================================================================

CREATE TABLE productos (
    nombre VARCHAR(255) PRIMARY KEY,
    marca VARCHAR(100),
    unidad VARCHAR(50) DEFAULT 'unidad',
    precio_actual NUMERIC(10, 2),
    precio_actualizado_en TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    
    -- Constraints
    CONSTRAINT nombre_no_vacio CHECK (length(trim(nombre)) > 0),
    CONSTRAINT precio_positivo CHECK (precio_actual IS NULL OR precio_actual >= 0),
    CONSTRAINT unidad_valida CHECK (unidad IN ('unidad', 'kg', 'g', 'l', 'ml'))
);

-- Ãndices para bÃºsqueda eficiente
CREATE INDEX idx_productos_marca ON productos(marca);
CREATE INDEX idx_productos_precio ON productos(precio_actual) WHERE precio_actual IS NOT NULL;
CREATE INDEX idx_productos_nombre_trgm ON productos USING gin(nombre gin_trgm_ops);
CREATE INDEX idx_productos_created_at ON productos(created_at DESC);

-- Comentarios
COMMENT ON TABLE productos IS 'CatÃ¡logo de productos Ãºnicos del supermercado';
COMMENT ON COLUMN productos.nombre IS 'Nombre normalizado del producto (PK)';
COMMENT ON COLUMN productos.unidad IS 'Unidad de medida: unidad, kg, g, l, ml';
COMMENT ON COLUMN productos.precio_actual IS 'Ãšltimo precio registrado del producto';
COMMENT ON COLUMN productos.precio_actualizado_en IS 'Fecha de la Ãºltima actualizaciÃ³n del precio';

-- =========================================================================
-- 5. TABLA: HISTORICO_PRECIOS
-- =========================================================================
-- Registro histÃ³rico de precios para anÃ¡lisis de inflaciÃ³n
-- PK Compuesta: (producto_nombre, fecha_vigencia)
-- =========================================================================

CREATE TABLE historico_precios (
    producto_nombre VARCHAR(255) NOT NULL,
    fecha_vigencia DATE NOT NULL,
    precio NUMERIC(10, 2) NOT NULL,
    fuente VARCHAR(50) DEFAULT 'ticket',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    
    -- Primary Key compuesta
    PRIMARY KEY (producto_nombre, fecha_vigencia),
    
    -- Foreign Keys
    CONSTRAINT fk_historico_producto 
        FOREIGN KEY (producto_nombre) 
        REFERENCES productos(nombre) 
        ON DELETE CASCADE
        ON UPDATE CASCADE,
    
    -- Constraints
    CONSTRAINT precio_historico_positivo CHECK (precio >= 0),
    CONSTRAINT fuente_valida CHECK (fuente IN ('ticket', 'manual', 'scraping_web', 'api'))
);

-- Ãndices optimizados para queries temporales
CREATE INDEX idx_historico_producto ON historico_precios(producto_nombre);
CREATE INDEX idx_historico_fecha ON historico_precios(fecha_vigencia DESC);
CREATE INDEX idx_historico_producto_fecha ON historico_precios(producto_nombre, fecha_vigencia DESC);

-- Comentarios
COMMENT ON TABLE historico_precios IS 'HistÃ³rico de precios de productos para anÃ¡lisis de inflaciÃ³n';
COMMENT ON COLUMN historico_precios.fecha_vigencia IS 'Fecha desde la cual el precio es vÃ¡lido';
COMMENT ON COLUMN historico_precios.fuente IS 'Origen del precio: ticket, manual, scraping_web, api';

-- =========================================================================
-- 6. TABLA: COMPRAS
-- =========================================================================
-- Tickets de compra del supermercado (sin el PDF)
-- PK: numero_factura (clave natural del ticket)
-- =========================================================================

CREATE TABLE compras (
    numero_factura VARCHAR(50) PRIMARY KEY,
    usuario_email VARCHAR(255) NOT NULL,
    fecha_hora TIMESTAMP NOT NULL,
    total NUMERIC(10, 2) NOT NULL,
    tienda VARCHAR(255),
    ubicacion VARCHAR(255),
    metodo_pago VARCHAR(50),
    numero_operacion VARCHAR(100),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    
    -- Foreign Keys
    CONSTRAINT fk_compras_usuario 
        FOREIGN KEY (usuario_email) 
        REFERENCES usuarios(email) 
        ON DELETE CASCADE
        ON UPDATE CASCADE,
    
    -- Constraints
    CONSTRAINT total_positivo CHECK (total >= 0),
    CONSTRAINT fecha_valida CHECK (fecha_hora <= CURRENT_TIMESTAMP),
    CONSTRAINT metodo_pago_valido CHECK (
        metodo_pago IS NULL OR 
        metodo_pago IN ('TARJETA BANCARIA', 'EFECTIVO', 'BIZUM', 'TRANSFERENCIA')
    )
);

-- Ãndices crÃ­ticos para rendimiento
CREATE INDEX idx_compras_usuario ON compras(usuario_email);
CREATE INDEX idx_compras_fecha ON compras(fecha_hora DESC);
CREATE INDEX idx_compras_usuario_fecha ON compras(usuario_email, fecha_hora DESC);
CREATE INDEX idx_compras_tienda ON compras(tienda);
CREATE INDEX idx_compras_total ON compras(total);

-- Comentarios
COMMENT ON TABLE compras IS 'Registro de tickets de compra (tabla ligera sin PDFs)';
COMMENT ON COLUMN compras.numero_factura IS 'NÃºmero de factura del ticket Mercadona (formato: XXXX-XXX-XXXXXX)';
COMMENT ON COLUMN compras.fecha_hora IS 'Fecha y hora exacta de la compra';
COMMENT ON COLUMN compras.numero_operacion IS 'NÃºmero de operaciÃ³n bancaria si aplica';

-- =========================================================================
-- 7. TABLA: TICKETS_PDF
-- =========================================================================
-- Almacenamiento de PDFs de tickets (tabla separada para rendimiento)
-- RelaciÃ³n 1:1 con COMPRAS
-- PK: numero_factura (FK hacia COMPRAS)
-- =========================================================================

CREATE TABLE tickets_pdf (
    numero_factura VARCHAR(50) PRIMARY KEY,
    ticket_pdf BYTEA NOT NULL,
    ticket_nombre_archivo VARCHAR(255) NOT NULL,
    ticket_tamano_bytes INTEGER NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    
    -- Foreign Key con CASCADE DELETE
    CONSTRAINT fk_tickets_compra 
        FOREIGN KEY (numero_factura) 
        REFERENCES compras(numero_factura) 
        ON DELETE CASCADE
        ON UPDATE CASCADE,
    
    -- Constraints
    CONSTRAINT tamano_positivo CHECK (ticket_tamano_bytes > 0),
    CONSTRAINT tamano_maximo CHECK (ticket_tamano_bytes <= 10485760), -- 10 MB mÃ¡ximo
    CONSTRAINT extension_pdf CHECK (ticket_nombre_archivo ILIKE '%.pdf')
);

-- Ãndices
CREATE INDEX idx_tickets_tamano ON tickets_pdf(ticket_tamano_bytes);
CREATE INDEX idx_tickets_created_at ON tickets_pdf(created_at DESC);

-- Comentarios
COMMENT ON TABLE tickets_pdf IS 'Almacenamiento de PDFs de tickets (tabla separada para optimizaciÃ³n)';
COMMENT ON COLUMN tickets_pdf.ticket_pdf IS 'Contenido binario del PDF del ticket';
COMMENT ON COLUMN tickets_pdf.ticket_tamano_bytes IS 'TamaÃ±o del PDF en bytes (mÃ¡x 10MB)';

-- =========================================================================
-- 8. TABLA: COMPRAS_PRODUCTOS
-- =========================================================================
-- RelaciÃ³n muchos-a-muchos entre COMPRAS y PRODUCTOS
-- PK Compuesta: (compra_numero_factura, producto_nombre)
-- =========================================================================

CREATE TABLE compras_productos (
    compra_numero_factura VARCHAR(50) NOT NULL,
    producto_nombre VARCHAR(255) NOT NULL,
    cantidad NUMERIC(10, 3) NOT NULL,
    precio_unitario NUMERIC(10, 2) NOT NULL,
    precio_total NUMERIC(10, 2) NOT NULL,
    descuento NUMERIC(10, 2) DEFAULT 0,
    iva_porcentaje NUMERIC(5, 2) NOT NULL,
    iva_importe NUMERIC(10, 2) NOT NULL,
    
    -- Primary Key compuesta
    PRIMARY KEY (compra_numero_factura, producto_nombre),
    
    -- Foreign Keys
    CONSTRAINT fk_compras_productos_compra 
        FOREIGN KEY (compra_numero_factura) 
        REFERENCES compras(numero_factura) 
        ON DELETE CASCADE
        ON UPDATE CASCADE,
    
    CONSTRAINT fk_compras_productos_producto 
        FOREIGN KEY (producto_nombre) 
        REFERENCES productos(nombre) 
        ON DELETE RESTRICT  -- No permitir borrar producto si estÃ¡ en compras
        ON UPDATE CASCADE,
    
    -- Constraints de validaciÃ³n
    CONSTRAINT cantidad_positiva CHECK (cantidad > 0),
    CONSTRAINT precio_unitario_positivo CHECK (precio_unitario >= 0),
    CONSTRAINT precio_total_positivo CHECK (precio_total >= 0),
    CONSTRAINT descuento_valido CHECK (descuento >= 0 AND descuento <= precio_total),
    CONSTRAINT iva_valido CHECK (iva_porcentaje >= 0 AND iva_porcentaje <= 100),
    CONSTRAINT iva_importe_valido CHECK (iva_importe >= 0),
    
    -- ValidaciÃ³n de coherencia de precios
    CONSTRAINT precio_coherente CHECK (
        abs(precio_total - (precio_unitario * cantidad - descuento)) < 0.01
    )
);

-- Ãndices para joins frecuentes
CREATE INDEX idx_compras_productos_compra ON compras_productos(compra_numero_factura);
CREATE INDEX idx_compras_productos_producto ON compras_productos(producto_nombre);
CREATE INDEX idx_compras_productos_cantidad ON compras_productos(cantidad);
CREATE INDEX idx_compras_productos_precio_total ON compras_productos(precio_total);

-- Comentarios
COMMENT ON TABLE compras_productos IS 'Productos incluidos en cada compra (relaciÃ³n M:N)';
COMMENT ON COLUMN compras_productos.cantidad IS 'Cantidad comprada (puede tener decimales para kg)';
COMMENT ON COLUMN compras_productos.descuento IS 'Descuento aplicado al producto';
COMMENT ON COLUMN compras_productos.iva_porcentaje IS 'Porcentaje de IVA (0%, 4%, 10%, 21%)';

-- =========================================================================
-- 9. TABLA: OBJETIVOS_AHORRO
-- =========================================================================
-- Metas de ahorro mensuales configuradas por el usuario
-- =========================================================================

CREATE TABLE objetivos_ahorro (
    id SERIAL PRIMARY KEY,
    usuario_email VARCHAR(255) NOT NULL,
    objetivo_mensual NUMERIC(10, 2) NOT NULL,
    mes DATE NOT NULL,
    conseguido BOOLEAN DEFAULT FALSE,
    ahorro_real NUMERIC(10, 2),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    
    -- Foreign Key
    CONSTRAINT fk_objetivos_usuario 
        FOREIGN KEY (usuario_email) 
        REFERENCES usuarios(email) 
        ON DELETE CASCADE
        ON UPDATE CASCADE,
    
    -- Constraints
    CONSTRAINT objetivo_positivo CHECK (objetivo_mensual > 0),
    CONSTRAINT mes_primer_dia CHECK (EXTRACT(DAY FROM mes) = 1),
    CONSTRAINT ahorro_real_valido CHECK (ahorro_real IS NULL OR ahorro_real >= 0),
    
    -- Un solo objetivo por usuario por mes
    CONSTRAINT unique_objetivo_usuario_mes UNIQUE (usuario_email, mes)
);

-- Ãndices
CREATE INDEX idx_objetivos_usuario ON objetivos_ahorro(usuario_email);
CREATE INDEX idx_objetivos_mes ON objetivos_ahorro(mes DESC);
CREATE INDEX idx_objetivos_conseguido ON objetivos_ahorro(conseguido);

-- Comentarios
COMMENT ON TABLE objetivos_ahorro IS 'Objetivos mensuales de ahorro definidos por el usuario';
COMMENT ON COLUMN objetivos_ahorro.mes IS 'Primer dÃ­a del mes (formato: YYYY-MM-01)';
COMMENT ON COLUMN objetivos_ahorro.conseguido IS 'Indica si se alcanzÃ³ el objetivo';
COMMENT ON COLUMN objetivos_ahorro.ahorro_real IS 'Ahorro real conseguido al final del mes';

-- =========================================================================
-- 10. TABLA: LOGROS
-- =========================================================================
-- CatÃ¡logo de logros desbloqueables (gamificaciÃ³n)
-- =========================================================================

CREATE TABLE logros (
    id SERIAL PRIMARY KEY,
    codigo VARCHAR(50) UNIQUE NOT NULL,
    nombre VARCHAR(255) NOT NULL,
    descripcion TEXT,
    icono VARCHAR(50),
    condicion_sql TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    
    -- Constraints
    CONSTRAINT codigo_formato CHECK (codigo ~ '^[A-Z_0-9]+$'),
    CONSTRAINT nombre_no_vacio CHECK (length(trim(nombre)) > 0)
);

-- Ãndices
CREATE INDEX idx_logros_codigo ON logros(codigo);

-- Comentarios
COMMENT ON TABLE logros IS 'CatÃ¡logo de logros desbloqueables en la aplicaciÃ³n';
COMMENT ON COLUMN logros.codigo IS 'CÃ³digo Ãºnico del logro (formato: MAYUSCULAS_CON_GUIONES)';
COMMENT ON COLUMN logros.condicion_sql IS 'Query SQL que verifica si el logro se desbloqueÃ³';

-- =========================================================================
-- 11. TABLA: LOGROS_USUARIO
-- =========================================================================
-- Logros desbloqueados por cada usuario
-- PK Compuesta: (usuario_email, logro_id)
-- =========================================================================

CREATE TABLE logros_usuario (
    usuario_email VARCHAR(255) NOT NULL,
    logro_id INTEGER NOT NULL,
    desbloqueado_en TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    metadata JSON,
    
    -- Primary Key compuesta
    PRIMARY KEY (usuario_email, logro_id),
    
    -- Foreign Keys
    CONSTRAINT fk_logros_usuario_usuario 
        FOREIGN KEY (usuario_email) 
        REFERENCES usuarios(email) 
        ON DELETE CASCADE
        ON UPDATE CASCADE,
    
    CONSTRAINT fk_logros_usuario_logro 
        FOREIGN KEY (logro_id) 
        REFERENCES logros(id) 
        ON DELETE CASCADE
        ON UPDATE CASCADE
);

-- Ãndices
CREATE INDEX idx_logros_usuario_usuario ON logros_usuario(usuario_email);
CREATE INDEX idx_logros_usuario_logro ON logros_usuario(logro_id);
CREATE INDEX idx_logros_usuario_fecha ON logros_usuario(desbloqueado_en DESC);

-- Comentarios
COMMENT ON TABLE logros_usuario IS 'Logros desbloqueados por cada usuario';
COMMENT ON COLUMN logros_usuario.metadata IS 'InformaciÃ³n adicional del logro (JSON)';

-- =========================================================================
-- 12. TABLA: PREFERENCIAS_USUARIO
-- =========================================================================
-- ConfiguraciÃ³n y preferencias de cada usuario
-- RelaciÃ³n 1:1 con USUARIOS
-- =========================================================================

CREATE TABLE preferencias_usuario (
    usuario_email VARCHAR(255) PRIMARY KEY,
    alertas_gasto_activas BOOLEAN DEFAULT TRUE,
    umbral_alerta_gasto NUMERIC(10, 2),
    notif_nuevos_logros BOOLEAN DEFAULT TRUE,
    notif_inflacion BOOLEAN DEFAULT TRUE,
    frecuencia_reportes VARCHAR(20) DEFAULT 'semanal',
    configuracion_extra JSON,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL,
    
    -- Foreign Key
    CONSTRAINT fk_preferencias_usuario 
        FOREIGN KEY (usuario_email) 
        REFERENCES usuarios(email) 
        ON DELETE CASCADE
        ON UPDATE CASCADE,
    
    -- Constraints
    CONSTRAINT umbral_positivo CHECK (umbral_alerta_gasto IS NULL OR umbral_alerta_gasto > 0),
    CONSTRAINT frecuencia_valida CHECK (
        frecuencia_reportes IN ('diaria', 'semanal', 'mensual', 'nunca')
    )
);

-- Comentarios
COMMENT ON TABLE preferencias_usuario IS 'Preferencias y configuraciÃ³n personalizada de cada usuario';
COMMENT ON COLUMN preferencias_usuario.umbral_alerta_gasto IS 'LÃ­mite de gasto para activar alertas';
COMMENT ON COLUMN preferencias_usuario.frecuencia_reportes IS 'Frecuencia de envÃ­o de reportes: diaria, semanal, mensual, nunca';

-- =========================================================================
-- 13. FUNCIONES Y TRIGGERS
-- =========================================================================

-- FunciÃ³n para actualizar automÃ¡ticamente el campo updated_at
CREATE OR REPLACE FUNCTION actualizar_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Aplicar trigger a las tablas que tienen updated_at
CREATE TRIGGER trigger_usuarios_updated_at
    BEFORE UPDATE ON usuarios
    FOR EACH ROW
    EXECUTE FUNCTION actualizar_updated_at();

CREATE TRIGGER trigger_preferencias_updated_at
    BEFORE UPDATE ON preferencias_usuario
    FOR EACH ROW
    EXECUTE FUNCTION actualizar_updated_at();

-- FunciÃ³n para actualizar el precio actual del producto
CREATE OR REPLACE FUNCTION actualizar_precio_producto()
RETURNS TRIGGER AS $$
BEGIN
    -- Actualizar el precio_actual del producto si es mÃ¡s reciente
    UPDATE productos
    SET 
        precio_actual = NEW.precio_unitario,
        precio_actualizado_en = CURRENT_TIMESTAMP
    WHERE 
        nombre = NEW.producto_nombre
        AND (precio_actualizado_en IS NULL OR precio_actualizado_en < CURRENT_TIMESTAMP);
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger para actualizar precio cuando se inserta en compras_productos
CREATE TRIGGER trigger_actualizar_precio_producto
    AFTER INSERT ON compras_productos
    FOR EACH ROW
    EXECUTE FUNCTION actualizar_precio_producto();

-- FunciÃ³n para registrar automÃ¡ticamente en el histÃ³rico de precios
CREATE OR REPLACE FUNCTION registrar_precio_historico()
RETURNS TRIGGER AS $$
BEGIN
    -- Insertar en histÃ³rico solo si no existe para esa fecha
    INSERT INTO historico_precios (producto_nombre, fecha_vigencia, precio, fuente)
    SELECT 
        NEW.producto_nombre,
        (SELECT fecha_hora::DATE FROM compras WHERE numero_factura = NEW.compra_numero_factura),
        NEW.precio_unitario,
        'ticket'
    ON CONFLICT (producto_nombre, fecha_vigencia) 
    DO UPDATE SET 
        precio = EXCLUDED.precio,
        created_at = CURRENT_TIMESTAMP
    WHERE historico_precios.fuente = 'ticket';  -- Solo actualizar si viene de ticket
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger para registrar histÃ³rico automÃ¡ticamente
CREATE TRIGGER trigger_registrar_precio_historico
    AFTER INSERT ON compras_productos
    FOR EACH ROW
    EXECUTE FUNCTION registrar_precio_historico();

-- =========================================================================
-- 14. VISTAS ÃšTILES
-- =========================================================================

-- Vista: EstadÃ­sticas mensuales por usuario
CREATE OR REPLACE VIEW estadisticas_mensuales AS
SELECT 
    DATE_TRUNC('month', fecha_hora)::DATE as mes,
    usuario_email,
    COUNT(*) as num_compras,
    SUM(total) as gasto_total,
    AVG(total) as gasto_medio,
    STDDEV(total) as desviacion_estandar,
    MIN(total) as compra_minima,
    MAX(total) as compra_maxima
FROM compras
GROUP BY DATE_TRUNC('month', fecha_hora), usuario_email;

COMMENT ON VIEW estadisticas_mensuales IS 'EstadÃ­sticas agregadas de compras por mes y usuario';

-- Vista: Top productos mÃ¡s comprados
CREATE OR REPLACE VIEW productos_top AS
SELECT 
    producto_nombre,
    COUNT(*) as veces_comprado,
    SUM(cantidad) as cantidad_total,
    SUM(precio_total) as gasto_total,
    AVG(precio_unitario) as precio_medio
FROM compras_productos
GROUP BY producto_nombre
ORDER BY veces_comprado DESC;

COMMENT ON VIEW productos_top IS 'Ranking de productos mÃ¡s comprados (global)';

-- =========================================================================
-- 15. DATOS DE EJEMPLO (LOGROS PREDEFINIDOS)
-- =========================================================================

INSERT INTO logros (codigo, nombre, descripcion, icono, condicion_sql) VALUES
('PRIMERA_COMPRA', 'Primera Compra', 'Has registrado tu primera compra en Mercastats', 'ðŸŽ‰', 
 'SELECT COUNT(*) >= 1 FROM compras WHERE usuario_email = $1'),

('COMPRAS_10', '10 Compras', 'Has registrado 10 compras', 'ðŸ›’', 
 'SELECT COUNT(*) >= 10 FROM compras WHERE usuario_email = $1'),

('COMPRAS_50', '50 Compras', 'Has registrado 50 compras', 'ðŸ†', 
 'SELECT COUNT(*) >= 50 FROM compras WHERE usuario_email = $1'),

('COMPRAS_100', 'Centenario', 'Has registrado 100 compras', 'ðŸ’¯', 
 'SELECT COUNT(*) >= 100 FROM compras WHERE usuario_email = $1'),

('AHORRO_MES', 'Ahorrador del Mes', 'Has cumplido tu objetivo de ahorro mensual', 'ðŸ’°', 
 'SELECT COUNT(*) >= 1 FROM objetivos_ahorro WHERE usuario_email = $1 AND conseguido = TRUE'),

('RACHA_SEMANAL', 'Racha Semanal', 'Has registrado compras durante 7 dÃ­as seguidos', 'ðŸ”¥', 
 NULL);

-- =========================================================================
-- 16. GRANTS Y PERMISOS (OPCIONAL)
-- =========================================================================

-- Si creas un usuario especÃ­fico para la aplicaciÃ³n:
-- CREATE USER mercastats_app WITH PASSWORD 'tu_password_seguro';
-- GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO mercastats_app;
-- GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO mercastats_app;
-- GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA public TO mercastats_app;

-- =========================================================================
-- 17. VERIFICACIÃ“N DE INTEGRIDAD
-- =========================================================================

-- Contar tablas creadas
SELECT 
    schemaname,
    tablename,
    pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) AS size
FROM pg_tables
WHERE schemaname = 'public'
ORDER BY tablename;

-- =========================================================================
-- FIN DEL SCRIPT
-- =========================================================================

-- Mensaje de Ã©xito
DO $$
BEGIN
    RAISE NOTICE 'âœ… Base de datos Mercastats creada exitosamente';
    RAISE NOTICE 'ðŸ“Š Tablas creadas: 12';
    RAISE NOTICE 'ðŸ”§ Funciones creadas: 3';
    RAISE NOTICE 'ðŸ“ˆ Vistas creadas: 2';
    RAISE NOTICE 'ðŸŽ¯ Logros predefinidos: 6';
    RAISE NOTICE '';
    RAISE NOTICE 'ðŸš€ Siguiente paso: Ejecutar migraciones de SQLx con:';
    RAISE NOTICE '   sqlx migrate run';
END $$;