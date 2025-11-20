# 🧪 OCR Warm-up Testing Guide

## Quick Start Testing

This guide helps you verify that the Python OCR warm-up is working correctly.

## Prerequisites

Before testing, ensure you have:

1. **PostgreSQL Running**:
   ```bash
   # Check if PostgreSQL is running
   pg_isready -U postgres
   ```

2. **Database Created**:
   ```bash
   psql -U postgres -c "CREATE DATABASE mercastats;"
   psql -U postgres mercastats -f sql/schema/schema.sql
   ```

3. **Python Dependencies Installed**:
   ```bash
   cd ocr-service
   pip install -r requirements.txt
   # Verify critical packages
   python -c "import pdfplumber; import pydantic; print('✅ Dependencies OK')"
   ```

4. **Environment Variables Set**:
   ```bash
   # Copy and configure .env
   cp .env.example .env
   # Edit DATABASE_URL and JWT_SECRET
   ```

## Test 1: Verify Warm-up Logs

### Expected Behavior
When starting the server, you should see warm-up logs **before** the database connection.

### Steps

1. **Start the server**:
   ```bash
   cd backend
   RUST_LOG=debug cargo run
   ```

2. **Expected Console Output** (in this exact order):
   ```
   INFO  tracing_subscriber: initializing logging...
   DEBUG Loading configuration from environment...
   INFO  Configuración cargada desde .env
   
   ⚡ WARM-UP PHASE STARTS HERE ⚡
   INFO  🐍 Warm-up: Inicializando intérprete de Python y cargando dependencias...
   
   [Wait 2-3 seconds - this is normal!]
   
   INFO  ✅ Warm-up: Módulos Python cargados y listos en memoria.
   ⚡ WARM-UP PHASE ENDS HERE ⚡
   
   INFO  Iniciando servidor en 127.0.0.1:8000
   DEBUG Connecting to PostgreSQL...
   INFO  Conectado a la base de datos
   INFO  Servidor escuchando en http://127.0.0.1:8000
   ```

3. **✅ PASS if**:
   - You see the 🐍 emoji message
   - There's a 2-3 second pause
   - You see the ✅ emoji message
   - Server starts successfully after warm-up

4. **❌ FAIL if**:
   - No warm-up messages appear
   - Server crashes with Python error
   - Warm-up happens after "Conectado a la base de datos"

## Test 2: Measure First Request Latency

### Expected Behavior
The first OCR request should be fast (~1 second), not slow (5-10 seconds).

### Steps

1. **Ensure server is freshly started** (restart if needed)

2. **Prepare a test ticket**:
   - Use a real Mercadona PDF ticket
   - Or use the test file: `ocr-service/test_response.json`

3. **Make a request immediately after startup**:

   **Option A: Using curl**
   ```bash
   # Convert PDF to base64
   base64 -w 0 ticket.pdf > ticket_b64.txt
   
   # Get JWT token first (adjust email/password)
   TOKEN=$(curl -s -X POST http://localhost:8000/api/auth/login \
     -H "Content-Type: application/json" \
     -d '{"email":"test@example.com","password":"password"}' \
     | jq -r '.token')
   
   # Time the OCR request
   time curl -X POST http://localhost:8000/api/ocr/process \
     -H "Content-Type: application/json" \
     -H "Authorization: Bearer $TOKEN" \
     -d @- <<EOF
   {
     "ticket_id": "test-$(date +%s)",
     "file_name": "ticket.pdf",
     "pdf_b64": "$(cat ticket_b64.txt)",
     "usuario_email": "test@example.com"
   }
   EOF
   ```

   **Option B: Using the Frontend**
   ```bash
   # Start frontend (in another terminal)
   cd frontend
   trunk serve --port 8080
   
   # Open browser at http://localhost:8080
   # Upload a ticket immediately after backend starts
   # Use browser DevTools Network tab to measure timing
   ```

4. **Measure the response time**:
   ```
   # Expected with warm-up
   real    0m0.850s  ✅ GOOD
   
   # Expected WITHOUT warm-up (old behavior)
   real    0m6.234s  ❌ BAD
   ```

5. **✅ PASS if**:
   - First request completes in < 2 seconds
   - No "module not found" errors
   - Valid JSON response with OCR data

6. **❌ FAIL if**:
   - First request takes > 5 seconds
   - Python import errors in logs
   - 500 Internal Server Error

## Test 3: Verify Fail-Fast Behavior

### Expected Behavior
If Python environment is broken, server should **refuse to start** (not crash later).

### Steps

1. **Break the Python environment**:
   ```bash
   # Temporarily rename the ocr-service directory
   cd /home/runner/work/mercastatsv2/mercastatsv2
   mv ocr-service ocr-service.backup
   ```

2. **Try to start the server**:
   ```bash
   cd backend
   cargo run
   ```

3. **Expected Output**:
   ```
   INFO  🐍 Warm-up: Inicializando intérprete de Python...
   ERROR ❌ Error CRÍTICO inicializando el worker de Python: No se pudo importar processor: No module named 'src'
   ERROR El servidor no puede arrancar sin el subsistema OCR.
   Error: No se pudo importar processor: No module named 'src'
   ```

4. **Server should NOT**:
   - Connect to the database
   - Start the HTTP server
   - Accept requests

5. **✅ PASS if**:
   - Server exits immediately with clear error
   - No database connection attempted
   - Error message mentions "worker de Python"

6. **❌ FAIL if**:
   - Server starts anyway
   - Crashes on first request instead of startup
   - No error message shown

7. **Restore the environment**:
   ```bash
   mv ocr-service.backup ocr-service
   ```

## Test 4: Performance Comparison

### Measure Startup Time

**Before Warm-up** (theoretical - old behavior):
```bash
# Startup: ~0.5 seconds
# First request: 5-10 seconds
# Total time to first successful OCR: ~10 seconds
```

**After Warm-up** (current implementation):
```bash
# Measure startup
time cargo run &
# Wait for "Servidor escuchando" message
# Note the time: should be ~3 seconds

# Measure first request
time curl -X POST ... (see Test 2)
# Note the time: should be <1 second

# Total time to first successful OCR: ~4 seconds ✅
```

**Interpretation**:
- Startup is slower (+2.5s) ✅ Expected
- First request is faster (-5s) ✅ Goal achieved!
- Overall better UX ✅ Success

## Troubleshooting

### Issue: Warm-up takes > 10 seconds

**Diagnosis**:
```bash
# Check if running on slow disk
df -h /home/runner/work/mercastatsv2
# Check disk I/O
iostat -x 1 5
```

**Solutions**:
- Use SSD for development
- Pre-install Python packages globally
- Check for antivirus interference (Windows)

### Issue: "No module named 'src'"

**Diagnosis**:
```bash
# Verify directory structure
ls -la /home/runner/work/mercastatsv2/mercastatsv2/ocr-service/src/

# Check Python can import manually
cd ocr-service
python -c "import sys; sys.path.insert(0, '.'); import src.processor"
```

**Solutions**:
- Ensure `ocr-service/src/` exists
- Check `__init__.py` files are present
- Verify relative path calculation in Rust

### Issue: Server starts but first request still slow

**Diagnosis**:
- Check if warm-up actually ran (look for 🐍 emoji in logs)
- Verify `load_processor_module()` is being called
- Check for lazy imports in Python code

**Debug**:
```rust
// Add to init_python_worker() for debugging
tracing::debug!("About to load processor module...");
let module = load_processor_module(py)?;
tracing::debug!("Module loaded: {:?}", module.name());
```

## Success Criteria Checklist

- [ ] ✅ Warm-up logs appear on startup
- [ ] ✅ Warm-up takes 2-3 seconds
- [ ] ✅ First OCR request takes < 2 seconds
- [ ] ✅ Subsequent requests remain fast
- [ ] ✅ Broken Python environment prevents startup
- [ ] ✅ No breaking changes to existing features

## Next Steps After Testing

1. **If all tests pass**:
   - Mark PR as ready for review
   - Update main documentation
   - Consider adding automated integration tests

2. **If tests fail**:
   - Review error messages carefully
   - Check Python environment setup
   - Verify file paths and permissions
   - Consult `docs/OCR_WARMUP_IMPLEMENTATION.md` troubleshooting section

3. **Optional enhancements**:
   - Add health check endpoint with Python status
   - Implement warm-up progress indicators
   - Add Docker optimization for faster container starts

---

**Last Updated**: 2025-11-20  
**Author**: GitHub Copilot  
**Related Docs**: `OCR_WARMUP_IMPLEMENTATION.md`
