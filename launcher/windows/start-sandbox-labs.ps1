[CmdletBinding()]
param(
    [string]$Distribution = "Ubuntu",
    [switch]$SkipDependencyInstall,
    [switch]$NoBrowser
)

$ErrorActionPreference = "Stop"
$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
Set-Location $ProjectRoot

if (-not (Get-Command wsl.exe -ErrorAction SilentlyContinue)) {
    throw "WSL2 no esta instalado. Ejecuta 'wsl --install' y reinicia Windows."
}

$DistributionNames = @(wsl.exe --list --quiet) | ForEach-Object { ($_ -replace "`0", "").Trim() } | Where-Object { $_ }
if ($Distribution -notin $DistributionNames) {
    throw "No existe la distribucion WSL '$Distribution'. Disponibles: $($DistributionNames -join ', ')"
}

$LinuxRoot = (wsl.exe -d $Distribution --cd $ProjectRoot -- pwd).Trim()
if (-not $LinuxRoot.StartsWith("/")) {
    throw "WSL no pudo traducir la ruta del repositorio: $ProjectRoot"
}

$CargoPath = (wsl.exe -d $Distribution -- sh -lc "command -v cargo 2>/dev/null").Trim()
if (-not $CargoPath) {
    throw "Rust no esta instalado en '$Distribution'. Instala rustup como indica docs/INSTALACION.md y vuelve a ejecutar el launcher."
}

$RequiredCommands = @("python3", "bwrap", "unshare", "prlimit", "systemd-run")
$Missing = @()
foreach ($CommandName in $RequiredCommands) {
    wsl.exe -d $Distribution -- sh -lc "command -v '$CommandName' >/dev/null 2>&1"
    if ($LASTEXITCODE -ne 0) { $Missing += $CommandName }
}

if ($Missing.Count -gt 0) {
    if ($SkipDependencyInstall) {
        throw "Faltan dependencias en WSL: $($Missing -join ', ')"
    }
    Write-Host "Instalando dependencias Linux que faltan: $($Missing -join ', ')"
    wsl.exe -d $Distribution -u root -- apt-get update
    if ($LASTEXITCODE -ne 0) { throw "apt-get update fallo dentro de WSL." }
    wsl.exe -d $Distribution -u root -- apt-get install -y bubblewrap util-linux python3
    if ($LASTEXITCODE -ne 0) { throw "No se pudieron instalar las dependencias dentro de WSL." }
}

$LinuxTarget = "$LinuxRoot/target/wsl"
Write-Host "Compilando sandboxctl dentro de WSL2..."
wsl.exe -d $Distribution --cd $LinuxRoot -- env "CARGO_TARGET_DIR=$LinuxTarget" $CargoPath build --release --locked -p sandboxctl
if ($LASTEXITCODE -ne 0) { throw "La compilacion Linux de sandboxctl fallo." }

$LinuxBinary = "$LinuxTarget/release/sandboxctl"
$RuntimeJson = wsl.exe -d $Distribution -- $LinuxBinary --root $LinuxRoot runtimes --json | Out-String
if ($LASTEXITCODE -ne 0) { throw "sandboxctl runtimes fallo dentro de WSL." }
$Runtimes = $RuntimeJson | ConvertFrom-Json
$Bwrap = @($Runtimes | Where-Object { $_.id -eq "bwrap" })[0]
if (-not $Bwrap.available) {
    throw "bubblewrap esta instalado pero el sondeo funcional fallo: $($Bwrap.detail)"
}
wsl.exe -d $Distribution -- $LinuxBinary --root $LinuxRoot doctor

corepack enable | Out-Null
pnpm dashboard:build
if ($LASTEXITCODE -ne 0) { throw "El build del panel fallo." }

$env:SANDBOX_LABS_WSL_DISTRO = $Distribution
$env:SANDBOX_LABS_WSL_ROOT = $LinuxRoot
$env:SANDBOXCTL_WSL_BIN = $LinuxBinary

$PortProbe = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 9093)
try {
    $PortProbe.Start()
} catch {
    throw "localhost:9093 ya esta ocupado. Cierra la instancia anterior de Sandbox Labs y vuelve a intentarlo."
} finally {
    $PortProbe.Stop()
}

$Node = (Get-Command node -ErrorAction Stop).Source
$Panel = Start-Process -FilePath $Node -ArgumentList "control-center/dist/server.js" -WorkingDirectory $ProjectRoot -WindowStyle Hidden -PassThru
try {
    $Ready = $false
    for ($Attempt = 0; $Attempt -lt 60; $Attempt++) {
        if ($Panel.HasExited) { throw "El panel termino durante el arranque con codigo $($Panel.ExitCode)." }
        try {
            $System = Invoke-RestMethod "http://127.0.0.1:9093/api/system" -TimeoutSec 1
            if ($System.backend.type -eq "wsl2") { $Ready = $true; break }
        } catch {
            Start-Sleep -Milliseconds 500
        }
    }
    if (-not $Ready) { throw "El panel no respondio con backend WSL2 en 30 segundos." }
    $Panel.Refresh()
    if ($Panel.HasExited) { throw "El panel termino durante el arranque con codigo $($Panel.ExitCode)." }
    if (-not $NoBrowser) { Start-Process "http://127.0.0.1:9093" }
    Write-Host "Sandbox Labs listo en http://127.0.0.1:9093 - backend WSL2/$Distribution"
    Wait-Process -Id $Panel.Id
} finally {
    if (-not $Panel.HasExited) { Stop-Process -Id $Panel.Id -Force }
    wsl.exe -d $Distribution -- $LinuxBinary --root $LinuxRoot service down --all
}
