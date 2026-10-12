param(
    [ValidateSet('dev', 'docker', 'test', 'setup', 'stop', 'help')]
    [string]$Mode = 'dev'
)

# Lanzador nativo de Windows. Requiere PowerShell 5.1+ y winget para instalar herramientas.
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot
Write-Host '==> Sistema detectado: Windows'

function Fail([string]$Message) { throw $Message }
function Has([string]$Command) { return [bool](Get-Command $Command -ErrorAction SilentlyContinue) }
function Refresh-Path {
    $env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' +
                [Environment]::GetEnvironmentVariable('Path', 'User') + ';' + $env:Path
    $env:Path = "$HOME\.cargo\bin;${env:ProgramFiles}\Tesseract-OCR;$env:Path"
}
function Install-Tool([string]$Id) {
    if (-not (Has 'winget')) { Fail "Falta winget (App Installer). Instálalo desde Microsoft Store y vuelve a ejecutar .\run.ps1." }
    Write-Host "==> Instalando $Id"
    & winget install --id $Id --exact --accept-package-agreements --accept-source-agreements
    if ($LASTEXITCODE -ne 0) { Fail "No se pudo instalar $Id con winget." }
    Refresh-Path
}
function Ensure-Command([string]$Command, [string]$Id) {
    if (-not (Has $Command)) { Install-Tool $Id }
    if (-not (Has $Command)) { Fail "Se instaló $Id, pero $Command no está disponible. Abre una terminal nueva y reintenta." }
}
function Env-Value([string]$Name, [string]$Default) {
    if (-not (Test-Path '.env')) { return $Default }
    $line = Get-Content '.env' | Where-Object { $_ -match "^$([regex]::Escape($Name))=" } | Select-Object -Last 1
    if ($line) { return ($line -split '=', 2)[1] }
    return $Default
}
function Ensure-Env {
    if (Test-Path '.env') { Ensure-TokenKey; return }
    $dbBytes = New-Object byte[] 16
    $jwtBytes = New-Object byte[] 32
    [Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($dbBytes)
    [Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($jwtBytes)
    $dbPassword = ([BitConverter]::ToString($dbBytes)).Replace('-', '').ToLowerInvariant()
    $jwtSecret = [Convert]::ToBase64String($jwtBytes)
    $content = [IO.File]::ReadAllText((Join-Path $PSScriptRoot '.env.example'))
    $content = $content.Replace('change_me_to_a_local_database_password', $dbPassword)
    $content = [regex]::Replace($content, '(?m)^JWT_SECRET=.*$', "JWT_SECRET=$jwtSecret")
    [IO.File]::WriteAllText((Join-Path $PSScriptRoot '.env'), $content)
    Write-Host '==> .env creado con secretos aleatorios'
    Ensure-TokenKey
}
function Ensure-TokenKey {
    if (Env-Value 'TOKEN_ENCRYPTION_KEY' '') { return }
    $bytes = New-Object byte[] 32
    [Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($bytes)
    $key = [Convert]::ToBase64String($bytes)
    $path = Join-Path $PSScriptRoot '.env'
    $content = [IO.File]::ReadAllText($path)
    if ($content -match '(?m)^TOKEN_ENCRYPTION_KEY=') {
        $content = [regex]::Replace($content, '(?m)^TOKEN_ENCRYPTION_KEY=.*$', "TOKEN_ENCRYPTION_KEY=$key")
    } else {
        $content += "`nTOKEN_ENCRYPTION_KEY=$key`n"
    }
    [IO.File]::WriteAllText($path, $content)
    Write-Host '==> TOKEN_ENCRYPTION_KEY generada'
}
function Apply-Migrations {
    $user = Env-Value 'POSTGRES_USER' 'mercastats_app'
    $db = Env-Value 'POSTGRES_DB' 'mercastats'
    foreach ($file in @(Get-ChildItem 'backend\migrations\0*.sql' | Sort-Object Name)) {
        if ($file.Name -match '^000[12]_') { continue }
        Get-Content $file.FullName -Raw | & docker exec -i -e 'PGOPTIONS=-c client_min_messages=warning' mercastats_db psql -v ON_ERROR_STOP=1 -q -U $user -d $db
        if ($LASTEXITCODE -ne 0) { Fail "No se pudo aplicar la migración $($file.Name)." }
    }
}
function Port-Busy([int]$Port) {
    $client = New-Object Net.Sockets.TcpClient
    try {
        $result = $client.BeginConnect('127.0.0.1', $Port, $null, $null)
        return ($result.AsyncWaitHandle.WaitOne(500) -and $client.Connected)
    } catch { return $false } finally { $client.Close() }
}
function Set-DbPort([int]$Old, [int]$New) {
    $content = [IO.File]::ReadAllText((Join-Path $PSScriptRoot '.env'))
    $content = [regex]::Replace($content, '(?m)^POSTGRES_PORT=.*$', "POSTGRES_PORT=$New")
    $content = $content.Replace("@localhost:$Old/", "@localhost:$New/")
    [IO.File]::WriteAllText((Join-Path $PSScriptRoot '.env'), $content)
}
function Ensure-Docker {
    Ensure-Command 'docker' 'Docker.DockerDesktop'
    & docker compose version *> $null
    if ($LASTEXITCODE -ne 0) { Fail 'Docker Compose v2 no está disponible. Actualiza Docker Desktop.' }
    & docker info *> $null
    if ($LASTEXITCODE -eq 0) { return }
    $desktop = Join-Path $env:ProgramFiles 'Docker\Docker\Docker Desktop.exe'
    if (-not (Test-Path $desktop)) { Fail 'Docker Desktop no responde. Inícialo y vuelve a ejecutar el script.' }
    Start-Process $desktop
    for ($i = 0; $i -lt 60; $i++) {
        Start-Sleep -Seconds 2
        & docker info *> $null
        if ($LASTEXITCODE -eq 0) { return }
    }
    Fail 'Docker Desktop no respondió en dos minutos.'
}
function Ensure-Db {
    Ensure-Docker
    $running = (& docker inspect -f '{{.State.Running}}' mercastats_db 2>$null)
    $port = [int](Env-Value 'POSTGRES_PORT' '5432')
    if ($running -ne 'true' -and (Port-Busy $port)) {
        $free = 5433..5437 | Where-Object { -not (Port-Busy $_) } | Select-Object -First 1
        if (-not $free) { Fail 'No hay puerto libre para PostgreSQL entre 5433 y 5437.' }
        Set-DbPort $port $free
        $port = $free
    }
    Write-Host '==> Levantando PostgreSQL'
    & docker compose up -d db
    if ($LASTEXITCODE -ne 0) { Fail 'No se pudo levantar PostgreSQL.' }
    for ($i = 0; $i -lt 45; $i++) {
        $health = (& docker inspect -f '{{.State.Health.Status}}' mercastats_db 2>$null)
        if ($health -eq 'healthy') {
            $env:PGPASSWORD = Env-Value 'POSTGRES_PASSWORD' ''
            $user = Env-Value 'POSTGRES_USER' 'mercastats_app'
            $db = Env-Value 'POSTGRES_DB' 'mercastats'
            $check = "import psycopg2; psycopg2.connect(host='127.0.0.1', port=$port, user='$user', dbname='$db', connect_timeout=5).close()"
            & '.\.venv\Scripts\python.exe' -c $check
            if ($LASTEXITCODE -ne 0) { Fail "La base de datos del proyecto no responde en localhost:$port." }
            Apply-Migrations
            return
        }
        Start-Sleep -Seconds 2
    }
    Fail 'PostgreSQL no llegó a estar sano. Revisa docker compose logs db.'
}
function Ensure-Python {
    if (-not (Has 'py')) { Install-Tool 'Python.Python.3.11' }
    & py -3.11 --version *> $null
    if ($LASTEXITCODE -ne 0) { Install-Tool 'Python.Python.3.11' }
    & py -3.11 --version *> $null
    if ($LASTEXITCODE -ne 0) { Fail 'Python 3.11 no está disponible mediante py -3.11.' }
    foreach ($item in @(
        @('.venv', 'intelligence-service\requirements.txt', 'pytest', 'httpx'),
        @('ocr-service\.venv', 'ocr-service\requirements.txt')
    )) {
        $venv = $item[0]; $requirements = $item[1]
        $python = Join-Path $venv 'Scripts\python.exe'
        if (-not (Test-Path $python)) {
            Write-Host "==> Creando $venv"
            & py -3.11 -m venv $venv
            if ($LASTEXITCODE -ne 0) { Fail "No se pudo crear $venv." }
        }
        $hash = (Get-FileHash $requirements -Algorithm SHA256).Hash
        $stamp = Join-Path $venv '.requirements.sha'
        $extras = @($item | Select-Object -Skip 2)
        $names = @(Get-Content $requirements) + $extras
        $names = @($names | ForEach-Object { ($_ -split '#', 2)[0] -replace '[<>=!~;\[\s].*$', '' } | Where-Object { $_ })
        $missing = $false
        foreach ($name in $names) {
            & $python -c 'import importlib.metadata,sys; importlib.metadata.version(sys.argv[1])' $name *> $null
            if ($LASTEXITCODE -ne 0) { $missing = $true; break }
        }
        if (-not (Test-Path $stamp) -or (Get-Content $stamp -Raw).Trim() -ne $hash -or $missing) {
            Write-Host "==> Instalando $requirements"
            & $python -m pip install -r $requirements @extras
            if ($LASTEXITCODE -ne 0) { Fail "Falló la instalación de $requirements." }
            Set-Content -Path $stamp -Value $hash
        }
    }
}
function Ensure-NativeTools {
    Ensure-Command 'node' 'OpenJS.NodeJS.LTS'
    Ensure-Command 'tesseract' 'UB-Mannheim.TesseractOCR'
    $langs = & tesseract --list-langs 2>&1
    if ($langs -notcontains 'spa') {
        $tessdata = Join-Path $PSScriptRoot '.tessdata'
        New-Item -ItemType Directory -Path $tessdata -Force | Out-Null
        foreach ($language in @('spa', 'eng')) {
            $file = Join-Path $tessdata "$language.traineddata"
            if (-not (Test-Path $file)) {
                Write-Host "==> Descargando datos OCR: $language"
                Invoke-WebRequest -Uri "https://github.com/tesseract-ocr/tessdata_fast/raw/refs/heads/main/$language.traineddata" -OutFile $file
            }
        }
        $env:TESSDATA_PREFIX = $tessdata
    }
    if (-not (Has 'rustup') -or -not (Has 'cargo')) { Install-Tool 'Rustlang.Rustup' }
    Refresh-Path
    if (-not (Has 'rustup') -or -not (Has 'cargo')) { Fail 'Rust no está disponible. Abre una terminal nueva y reintenta.' }
    $targets = & rustup target list --installed
    if ($targets -notcontains 'wasm32-unknown-unknown') {
        & rustup target add wasm32-unknown-unknown
        if ($LASTEXITCODE -ne 0) { Fail 'No se pudo instalar el objetivo WebAssembly.' }
    }
    if (-not (Has 'trunk')) {
        & cargo install --locked trunk
        if ($LASTEXITCODE -ne 0) { Fail 'No se pudo instalar Trunk.' }
    }
    $hash = (Get-FileHash 'Cargo.lock' -Algorithm SHA256).Hash
    $stamp = '.cargo-fetch.sha'
    if (-not (Test-Path $stamp) -or (Get-Content $stamp -Raw).Trim() -ne $hash) {
        & cargo fetch --locked
        if ($LASTEXITCODE -ne 0) { Fail 'No se pudieron descargar las dependencias Rust.' }
        Set-Content -Path $stamp -Value $hash
    }
}
function Prepare {
    Ensure-Env
    Ensure-NativeTools
    Ensure-Python
    Ensure-Db
}

switch ($Mode) {
    'help' { Write-Host '.\run.ps1 [dev|docker|test|setup|stop|help]'; break }
    'docker' {
        Ensure-Env; Ensure-Docker
        & docker compose up -d --wait db
        if ($LASTEXITCODE -ne 0) { Fail 'No se pudo levantar PostgreSQL.' }
        Apply-Migrations
        & docker compose up --build
        exit $LASTEXITCODE
    }
    'stop' {
        Ensure-Docker
        & docker compose down
        exit $LASTEXITCODE
    }
    'setup' { Prepare; Write-Host 'Entorno listo. Lanza .\run.ps1'; break }
    'test' {
        Prepare
        & '.\.venv\Scripts\python.exe' -m pytest -q intelligence-service
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
        $user = Env-Value 'POSTGRES_USER' 'mercastats_app'
        $password = Env-Value 'POSTGRES_PASSWORD' ''
        $db = Env-Value 'POSTGRES_DB' 'mercastats'
        $port = Env-Value 'POSTGRES_PORT' '5432'
        $env:DATABASE_URL = "postgres://${user}:${password}@localhost:${port}/${db}"
        & cargo test -p mercastats-backend
        exit $LASTEXITCODE
    }
    'dev' {
        Prepare
        foreach ($port in @(3000, 8000, 8001)) {
            if (Port-Busy $port) { Fail "El puerto $port está ocupado." }
        }
        Write-Host 'Aplicación: http://localhost:3000  API: http://localhost:8000/health'
        & node dev.js
        exit $LASTEXITCODE
    }
}
