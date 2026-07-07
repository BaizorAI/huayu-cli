# package.ps1 — Build and package huayu for Windows x64
# Outputs zip directly to .\release\ (*.zip gitignored), run deploy.sh to push.
#
# Usage:
#   .\package.ps1              # full build + package
#   .\package.ps1 -SkipBuild   # reuse existing target\release\huayu.exe

param(
    [switch]$SkipBuild  # skip cargo build, use existing target\release\huayu.exe
)

$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$Triple     = "x86_64-pc-windows-msvc"
$ZipName    = "huayu-$Triple.zip"
$ReleaseDir = "$PSScriptRoot\release"
$ZipOut     = "$ReleaseDir\$ZipName"
$ToolsDir   = "$env:USERPROFILE\.huayu\tools"

# ── helpers ────────────────────────────────────────────────────────────────

function Step([string]$msg) { Write-Host "  $msg" -ForegroundColor Cyan }
function Ok([string]$msg)   { Write-Host "  [ok] $msg" -ForegroundColor Green }
function Warn([string]$msg) { Write-Host "  [!]  $msg" -ForegroundColor Yellow }
function Fail([string]$msg) { Write-Host "`n  [error] $msg`n" -ForegroundColor Red; exit 1 }

# ── version from Cargo.toml ────────────────────────────────────────────────

$cargoToml = Get-Content "$PSScriptRoot\Cargo.toml" -Raw
if ($cargoToml -match 'version\s*=\s*"([^"]+)"') {
    $Version = $Matches[1]
} else {
    Fail "Could not read version from Cargo.toml"
}

Write-Host ""
Write-Host "  huayu $Version — Windows x64 package" -ForegroundColor White
Write-Host "  ─────────────────────────────────────────────────────" -ForegroundColor DarkGray

# ── build ──────────────────────────────────────────────────────────────────

if ($SkipBuild) {
    Step "Skipping cargo build (-SkipBuild)"
} else {
    Step "cargo build --release ..."
    Push-Location $PSScriptRoot
    cargo build --release
    $rc = $LASTEXITCODE
    Pop-Location
    if ($rc -ne 0) { Fail "cargo build failed (exit $rc)" }
    Ok "Build complete"
}

$ExePath = "$PSScriptRoot\target\release\huayu.exe"
if (-not (Test-Path $ExePath)) {
    Fail "huayu.exe not found at $ExePath — run without -SkipBuild"
}

# ── stage area ─────────────────────────────────────────────────────────────

$Stage = Join-Path $env:TEMP "huayu-stage-$(New-Guid)"
New-Item -ItemType Directory -Path "$Stage\tools" -Force | Out-Null

Copy-Item $ExePath "$Stage\huayu.exe" -Force
Ok "huayu.exe  ($([Math]::Round((Get-Item $ExePath).Length / 1MB, 1)) MB)"

# codex.exe
$codexExe = "$ToolsDir\codex.exe"
if (Test-Path $codexExe) {
    Copy-Item $codexExe "$Stage\tools\codex.exe" -Force
    $codexVer = "$ToolsDir\codex.version"
    if (Test-Path $codexVer) {
        Copy-Item $codexVer "$Stage\tools\codex.version" -Force
        Ok "codex.exe    v$((Get-Content $codexVer -Raw).Trim())"
    } else {
        Ok "codex.exe"
    }
} else {
    Warn "codex.exe not found — run 'huayu update codex' first"
}

# claude*
$claudeFiles = @(Get-ChildItem "$ToolsDir\claude*" -ErrorAction SilentlyContinue)
if ($claudeFiles.Count -gt 0) {
    foreach ($f in $claudeFiles) {
        Copy-Item $f.FullName "$Stage\tools\$($f.Name)" -Force
    }
    $claudeVer = "$ToolsDir\claude.version"
    $vStr = if (Test-Path $claudeVer) { " v$((Get-Content $claudeVer -Raw).Trim())" } else { "" }
    Ok "claude$vStr"
} else {
    Warn "claude not found — run 'huayu update claude' first"
}

# bash (minimal set for Claude Code POSIX shell requirement on Windows)
$gitBash = "C:\Program Files\Git\usr\bin\bash.exe"
$gitMsys = "C:\Program Files\Git\usr\bin\msys-2.0.dll"
if ((Test-Path $gitBash) -and (Test-Path $gitMsys)) {
    New-Item -ItemType Directory -Path "$Stage\tools\bash" -Force | Out-Null
    Copy-Item $gitBash "$Stage\tools\bash\bash.exe" -Force
    Copy-Item $gitMsys "$Stage\tools\bash\msys-2.0.dll" -Force
    $bashKB = [Math]::Round(((Get-Item $gitBash).Length + (Get-Item $gitMsys).Length) / 1KB, 0)
    Ok "bash.exe + msys-2.0.dll  ($bashKB KB)"
} else {
    Warn "Git bash not found — Claude mode may fail on machines without Git"
}

# ── zip → release\ ─────────────────────────────────────────────────────────

# Kill any running huayu instances so the exe isn't locked during zipping.
Get-Process -Name huayu -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue

New-Item -ItemType Directory -Path $ReleaseDir -Force | Out-Null
Step "Creating $ZipName ..."
if (Test-Path $ZipOut) { Remove-Item $ZipOut -Force }
Compress-Archive -Path "$Stage\*" -DestinationPath $ZipOut -Force
Remove-Item -Recurse -Force $Stage -ErrorAction SilentlyContinue
Ok "$ZipName  ($([Math]::Round((Get-Item $ZipOut).Length / 1MB, 1)) MB)"

# ── version file ──────────────────────────────────────────────────────────

[System.IO.File]::WriteAllText("$ReleaseDir\huayu-version.txt", "$Version`n")
Ok "huayu-version.txt ($Version)"

# ── summary ───────────────────────────────────────────────────────────────

Write-Host ""
Write-Host "  ─────────────────────────────────────────────────────" -ForegroundColor DarkGray
Write-Host "  release\ contents:" -ForegroundColor Green
Get-ChildItem $ReleaseDir | ForEach-Object {
    $kb = [Math]::Round($_.Length / 1KB, 0)
    Write-Host "    $($_.Name.PadRight(45)) $kb KB"
}
Write-Host ""
Write-Host "  Next: " -NoNewline; Write-Host "./deploy.sh" -ForegroundColor White
Write-Host ""
