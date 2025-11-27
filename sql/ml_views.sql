-- Vista para features de tickets (Modelos A y B)
CREATE OR REPLACE VIEW ml_ticket_features AS
WITH compras_ordenadas AS (
    SELECT
        c.numero_factura,
        c.usuario_email,
        c.fecha_hora,
        c.total,
        LAG(c.fecha_hora) OVER (
            PARTITION BY c.usuario_email
            ORDER BY c.fecha_hora
        ) AS prev_fecha_hora
    FROM compras c
),
compras_con_delta AS (
    SELECT
        numero_factura,
        usuario_email,
        fecha_hora,
        total,
        EXTRACT(EPOCH FROM (fecha_hora - prev_fecha_hora)) / 86400.0
            AS days_since_last_shop
    FROM compras_ordenadas
),
compras_con_acumulados AS (
    SELECT
        c.numero_factura,
        c.usuario_email,
        c.fecha_hora,
        c.total,
        c.days_since_last_shop,
        -- gasto ultimos 30 dias
        (
            SELECT COALESCE(SUM(c2.total), 0.0)
            FROM compras c2
            WHERE c2.usuario_email = c.usuario_email
              AND c2.fecha_hora BETWEEN c.fecha_hora - INTERVAL '30 days' AND c.fecha_hora
              AND c2.numero_factura != c.numero_factura 
              AND c2.fecha_hora < c.fecha_hora
        ) AS total_last_30d,
        -- n tickets ultimos 30 dias
        (
            SELECT COUNT(*)
            FROM compras c3
            WHERE c3.usuario_email = c.usuario_email
              AND c3.fecha_hora BETWEEN c.fecha_hora - INTERVAL '30 days' AND c.fecha_hora
              AND c3.fecha_hora < c.fecha_hora
        ) AS tickets_last_30d
    FROM compras_con_delta c
)
SELECT
    numero_factura,
    usuario_email,
    fecha_hora,
    total,
    -- Features temporales
    EXTRACT(DOW FROM fecha_hora)      AS day_of_week,
    EXTRACT(DAY FROM fecha_hora)      AS day_of_month,
    EXTRACT(HOUR FROM fecha_hora)     AS hour_of_day,
    COALESCE(days_since_last_shop, 0) AS days_since_last_shop,
    total_last_30d,
    tickets_last_30d,
    -- Ejemplo de "is_payday_week": primera semana del mes
    (EXTRACT(DAY FROM fecha_hora) BETWEEN 1 AND 7) AS is_payday_week
FROM compras_con_acumulados;

-- Vista global de estadisticas de productos
CREATE OR REPLACE VIEW ml_product_stats AS
SELECT
    cp.producto_nombre,
    COUNT(*)                     AS times_bought,
    SUM(cp.cantidad)            AS total_quantity,
    SUM(cp.precio_total)        AS total_spent,
    AVG(cp.precio_unitario)     AS avg_unit_price
FROM compras_productos cp
GROUP BY cp.producto_nombre;

-- Vista de estadisticas de productos por usuario y dia (Modelo C)
CREATE OR REPLACE VIEW ml_user_product_stats AS
SELECT
    c.usuario_email,
    cp.producto_nombre,
    EXTRACT(DOW FROM c.fecha_hora) AS day_of_week,
    COUNT(*)                       AS times_bought,
    SUM(cp.cantidad)               AS total_quantity,
    SUM(cp.precio_total)           AS total_spent
FROM compras_productos cp
JOIN compras c
  ON cp.compra_numero_factura = c.numero_factura
GROUP BY c.usuario_email, cp.producto_nombre, EXTRACT(DOW FROM c.fecha_hora);
