-- =========================================================================
-- MERCASTATS - Fotos de tickets en papel
-- =========================================================================
-- IDEMPOTENTE (ver 0003): ./run.sh la reaplica en cada arranque.
--
-- Los tickets en papel se suben como foto y se guardan junto a los PDF en
-- tickets_pdf, así que el nombre del archivo puede terminar también en una
-- extensión de imagen. Para los PDF la regla no cambia (terminar en .pdf).
-- =========================================================================

ALTER TABLE tickets_pdf DROP CONSTRAINT IF EXISTS extension_pdf;
ALTER TABLE tickets_pdf DROP CONSTRAINT IF EXISTS extension_valida;
ALTER TABLE tickets_pdf ADD CONSTRAINT extension_valida
    CHECK (ticket_nombre_archivo ~* '\.(pdf|jpe?g|png|webp|heic|heif)$');

COMMENT ON TABLE tickets_pdf IS 'Archivo original de cada ticket: PDF digital o foto del ticket en papel';
