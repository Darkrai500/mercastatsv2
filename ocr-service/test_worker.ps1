# Script de prueba para el worker OCR
# Convierte el PDF de ejemplo a base64 y lo envía al endpoint

$pdfPath = "..\docs\20230810 Mercadona 52,11 €.pdf"

Write-Host "Leyendo PDF de ejemplo..." -ForegroundColor Cyan
$bytes = [System.IO.File]::ReadAllBytes((Resolve-Path $pdfPath))
$base64 = [System.Convert]::ToBase64String($bytes)

Write-Host "PDF convertido a base64. Tamaño: $($bytes.Length) bytes" -ForegroundColor Green

# Crear JSON
$body = @{
    ticket_id = "test-123-456"
    file_name = "ticket_mercadona_test.pdf"
    pdf_b64 = $base64
} | ConvertTo-Json

Write-Host "`nEnviando request a http://127.0.0.1:9000/process-ticket..." -ForegroundColor Cyan

try {
    $response = Invoke-RestMethod -Uri "http://127.0.0.1:9000/process-ticket" -Method POST -Body $body -ContentType "application/json"

    Write-Host "`n✅ SUCCESS!" -ForegroundColor Green
    Write-Host "==================== RESPUESTA ====================" -ForegroundColor Yellow
    Write-Host "Ticket ID:        $($response.ticket_id)" -ForegroundColor White
    Write-Host "Número Factura:   $($response.numero_factura)" -ForegroundColor White
    Write-Host "Fecha:            $($response.fecha)" -ForegroundColor White
    Write-Host "Total:            $($response.total) €" -ForegroundColor White
    Write-Host "`nRaw Text (preview):" -ForegroundColor Yellow
    Write-Host $response.raw_text.Substring(0, [Math]::Min(300, $response.raw_text.Length)) -ForegroundColor Gray
    Write-Host "..." -ForegroundColor Gray
    Write-Host "===================================================" -ForegroundColor Yellow
} catch {
    Write-Host "`n❌ ERROR!" -ForegroundColor Red
    Write-Host $_.Exception.Message -ForegroundColor Red
}
