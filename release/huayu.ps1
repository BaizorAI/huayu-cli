# huayu Windows installer
# Usage: irm https://baizor.com/install/huayu.ps1 | iex
#
# What this script does:
#   1. Downloads the latest huayu bundle from baizor.com/install/
#      (includes huayu.exe, codex.exe, claude) — no Node.js or npm required.
#   2. Extracts everything to %USERPROFILE%\.huayu\
#   3. Adds %USERPROFILE%\.huayu\bin to your User PATH.
#   4. Prints next steps.

[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$ErrorActionPreference = 'Stop'

# ── Config ─────────────────────────────────────────────────────────────────

$BaseUrl     = "https://baizor.com/install"
$huayuHome = "$env:USERPROFILE\.huayu"
$BinDir      = "$huayuHome\bin"
$ToolsDir    = "$huayuHome\tools"

# ── Helpers ────────────────────────────────────────────────────────────────

function Write-Step([string]$msg) { Write-Host "  $msg" -ForegroundColor Cyan }
function Write-Ok([string]$msg)   { Write-Host "  [ok] $msg" -ForegroundColor Green }
function Write-Warn([string]$msg) { Write-Host "  [!]  $msg" -ForegroundColor Yellow }
function Fail([string]$msg) {
    Write-Host ""
    Write-Host "  [error] $msg" -ForegroundColor Red
    Write-Host ""
    exit 1
}

# ── Arch check ─────────────────────────────────────────────────────────────

if ($env:PROCESSOR_ARCHITECTURE -ne "AMD64") {
    Fail "Only x64 Windows is supported at this time (detected: $env:PROCESSOR_ARCHITECTURE)."
}

# ── Fetch version ──────────────────────────────────────────────────────────

Write-Host ""
Write-Host "huayu installer" -ForegroundColor White
Write-Host "─────────────────────────────────────────────────────────" -ForegroundColor DarkGray
Write-Step "Fetching latest version from baizor.com ..."

try {
    $version = (Invoke-RestMethod -Uri "$BaseUrl/huayu-version.txt" -UseBasicParsing).Trim()
} catch {
    Fail "Could not reach baizor.com: $_`n       Check your internet connection or try again later."
}

Write-Ok "Latest version: $version"

# ── Download bundle ────────────────────────────────────────────────────────

$zipName    = "huayu-x86_64-pc-windows-msvc.zip"
$downloadUrl = "$BaseUrl/$zipName"

$tempDir = [System.IO.Path]::Combine([System.IO.Path]::GetTempPath(), [System.Guid]::NewGuid())
New-Item -ItemType Directory -Path $tempDir -Force | Out-Null
$zipPath = "$tempDir\$zipName"

Write-Step "Downloading $zipName ..."
try {
    Invoke-WebRequest -Uri $downloadUrl -OutFile $zipPath -UseBasicParsing
} catch {
    Remove-Item -Recurse -Force $tempDir -ErrorAction SilentlyContinue
    Fail "Download failed: $_`n       URL: $downloadUrl"
}

$sizeMb = [Math]::Round((Get-Item $zipPath).Length / 1MB, 1)
Write-Ok "Downloaded ${sizeMb} MB"

# ── Extract ────────────────────────────────────────────────────────────────

Write-Step "Extracting to $huayuHome ..."

New-Item -ItemType Directory -Path $BinDir   -Force | Out-Null
New-Item -ItemType Directory -Path $ToolsDir -Force | Out-Null

try {
    $extractDir = "$tempDir\extracted"
    Expand-Archive -Path $zipPath -DestinationPath $extractDir -Force

    # huayu.exe → bin\
    $huayuExe = Get-ChildItem -Path $extractDir -Filter "huayu.exe" -Recurse |
                  Select-Object -First 1
    if (-not $huayuExe) { throw "huayu.exe not found in archive" }
    Copy-Item -Path $huayuExe.FullName -Destination "$BinDir\huayu.exe" -Force
    Write-Ok "huayu.exe  ->  $BinDir"

    # tools\codex.exe → tools\
    $codexExe = Get-ChildItem -Path $extractDir -Filter "codex.exe" -Recurse |
                Select-Object -First 1
    if ($codexExe) {
        Copy-Item -Path $codexExe.FullName -Destination "$ToolsDir\codex.exe" -Force
        Write-Ok "codex.exe    ->  $ToolsDir"
    } else {
        Write-Warn "codex.exe not found in archive; run 'huayu update codex' after install."
    }

    # tools\claude* → tools\
    $claudeFiles = Get-ChildItem -Path $extractDir -Filter "claude*" -Recurse
    foreach ($f in $claudeFiles) {
        Copy-Item -Path $f.FullName -Destination "$ToolsDir\$($f.Name)" -Force
    }
    if ($claudeFiles.Count -gt 0) {
        Write-Ok "claude       ->  $ToolsDir"
    } else {
        Write-Warn "claude not found in archive; run 'huayu update claude' after install."
    }

    # version markers
    [System.IO.File]::WriteAllText("$ToolsDir\huayu.version", $version)

} catch {
    Remove-Item -Recurse -Force $tempDir -ErrorAction SilentlyContinue
    Fail "Extraction failed: $_"
} finally {
    Remove-Item -Recurse -Force $tempDir -ErrorAction SilentlyContinue
}

# ── PATH ───────────────────────────────────────────────────────────────────

$userPath = [Environment]::GetEnvironmentVariable("PATH", "User")
if ($null -eq $userPath) { $userPath = "" }

if (($userPath -split ";") -notcontains $BinDir) {
    [Environment]::SetEnvironmentVariable("PATH", ($userPath.TrimEnd(";") + ";$BinDir"), "User")
    Write-Ok "Added $BinDir to User PATH"
} else {
    Write-Ok "$BinDir already in PATH"
}

# Make huayu available in the current session without restarting
$env:PATH = "$env:PATH;$BinDir"

# ── Auto-install tools (codex + claude from baizor.com) ────────────────────

function Install-Tool([string]$Name, [string]$ToolVersion) {
    $zipName = "$Name-$ToolVersion-x86_64-pc-windows-msvc.zip"
    $url     = "$BaseUrl/$zipName"
    $tmpZip  = [System.IO.Path]::Combine([System.IO.Path]::GetTempPath(), $zipName)
    Write-Step "Downloading $zipName ..."
    try {
        Invoke-WebRequest -Uri $url -OutFile $tmpZip -UseBasicParsing
        Write-Step "Extracting $Name $ToolVersion ..."
        Expand-Archive -Path $tmpZip -DestinationPath $ToolsDir -Force
        Write-Ok "$Name $ToolVersion"
    } catch {
        Write-Warn "$Name download failed: $($_.Exception.Message)"
        Write-Host "    Run 'huayu update' after launch to retry." -ForegroundColor DarkGray
    } finally {
        Remove-Item $tmpZip -ErrorAction SilentlyContinue
    }
}

function Get-ToolVersion([string]$Name, [string]$Default) {
    try {
        return (Invoke-RestMethod -Uri "$BaseUrl/$Name-version.txt" -UseBasicParsing).Trim()
    } catch {
        return $Default
    }
}

Write-Host ""
$CodexVersion  = Get-ToolVersion "codex"  "0.142.5"
$ClaudeVersion = Get-ToolVersion "claude" "1.0.3"
Install-Tool "codex"  $CodexVersion
Install-Tool "claude" $ClaudeVersion

# ── Codex model config ─────────────────────────────────────────────────────

$CodexConfigDir  = "$env:USERPROFILE\.codex"
$CodexConfigFile = "$CodexConfigDir\config.toml"
if (-not (Test-Path $CodexConfigFile)) {
    New-Item -ItemType Directory -Path $CodexConfigDir -Force | Out-Null
    $configContent = @'
[model_info."huayu-v1"]
context_window = 128000
max_output_tokens = 16384

[model_info."huayu-fable-5"]
context_window = 128000
max_output_tokens = 16384

[model_info."huayu3.6-35b"]
context_window = 32768
max_output_tokens = 8192
'@
    [System.IO.File]::WriteAllText($CodexConfigFile, $configContent)
    Write-Ok "Created $CodexConfigFile"
}

# ── Done ───────────────────────────────────────────────────────────────────

Write-Host ""
Write-Host "─────────────────────────────────────────────────────────" -ForegroundColor DarkGray
Write-Host "  huayu $version installed!" -ForegroundColor Green
Write-Host ""
Write-Host "  Start:   " -NoNewline; Write-Host "huayu" -ForegroundColor White
Write-Host "  Login:   " -NoNewline; Write-Host "huayu login" -ForegroundColor White
Write-Host ""
Write-Host "  If 'huayu' is not found, open a new terminal window." -ForegroundColor DarkGray
Write-Host ""
