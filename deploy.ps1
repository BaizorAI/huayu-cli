# deploy.ps1 — Smart deploy: only scp components with version changes
# Compares local .build-state.json vs remote *-version.txt on baizor.
#
# Usage:
#   .\deploy.ps1            # deploy changed components
#   .\deploy.ps1 -DryRun    # show diff only
#   .\deploy.ps1 -Force     # deploy all regardless of version

param(
    [switch]$DryRun,
    [switch]$Force
)

$ErrorActionPreference = 'Stop'
$ScriptDir  = $PSScriptRoot
$ReleaseDir = "$ScriptDir\release"
$StateFile  = "$ScriptDir\.build-state.json"
$RemoteHost = "baizor"
$RemotePath = "/lucky/NewApi/data/install"

# ── Helpers ────────────────────────────────────────────────────────────────

function Step([string]$msg)   { Write-Host "  $msg" -ForegroundColor Cyan }
function Ok([string]$msg)     { Write-Host "  [ok] $msg" -ForegroundColor Green }
function Skip([string]$msg)   { Write-Host "  [skip] $msg" -ForegroundColor DarkGray }
function Deploy([string]$msg) { Write-Host "  [deploy] $msg" -ForegroundColor Yellow }
function Fail([string]$msg)   { Write-Host "`n  [error] $msg`n" -ForegroundColor Red; exit 1 }

# ── Read local build state ─────────────────────────────────────────────────

if (-not (Test-Path $StateFile)) {
    Fail ".build-state.json not found — run build.ps1 first"
}
$State = Get-Content $StateFile -Raw | ConvertFrom-Json

# ── Read remote versions via SSH ───────────────────────────────────────────

function Get-RemoteVersion([string]$Name) {
    try {
        $ver = ssh $RemoteHost "cat $RemotePath/$Name-version.txt 2>/dev/null" 2>$null
        if ($ver) { return $ver.Trim() }
    } catch {}
    return ""
}

# ── Main ───────────────────────────────────────────────────────────────────

Write-Host ""
Write-Host "  huayu deploy system" -ForegroundColor White
Write-Host "  ─────────────────────────────────────────────────────" -ForegroundColor DarkGray
if ($DryRun) { Write-Host "  (dry run — no files will be copied)" -ForegroundColor DarkGray }
Write-Host ""

Step "Checking remote versions on $RemoteHost ..."

$components = @("huayu", "codex", "claude", "skills")
$toDeploy = @()

foreach ($name in $components) {
    $localVer  = if ($State.PSObject.Properties[$name]) { $State.$name.version } else { "" }
    $remoteVer = Get-RemoteVersion $name

    if (-not $localVer) {
        Skip "$name — not built locally"
        continue
    }

    if (-not $Force -and $localVer -eq $remoteVer) {
        Skip "$name  local=$localVer  remote=$remoteVer"
        continue
    }

    $remoteDisplay = if ($remoteVer) { $remoteVer } else { "(none)" }
    Deploy "$name  local=$localVer  remote=$remoteDisplay"
    $toDeploy += $name
}

Write-Host ""

if ($toDeploy.Count -eq 0) {
    Write-Host "  All versions match remote — nothing to deploy." -ForegroundColor DarkGray
    Write-Host ""
    exit 0
}

if ($DryRun) {
    Write-Host "  $($toDeploy.Count) component(s) would be deployed: $($toDeploy -join ', ')" -ForegroundColor Yellow
    Write-Host ""
    exit 0
}

# ── Deploy files ───────────────────────────────────────────────────────────

if (-not (Test-Path $ReleaseDir)) {
    Fail "release/ directory not found — run build.ps1 first"
}

$Triple = "x86_64-pc-windows-msvc"
$LinuxTriple = "x86_64-unknown-linux-musl"
$LinuxToolsTriple = "x86_64-unknown-linux-gnu"

foreach ($name in $toDeploy) {
    $ver = $State.$name.version

    switch ($name) {
        "huayu" {
            $files = @(
                "$ReleaseDir\huayu-$Triple.zip"
                "$ReleaseDir\huayu-version.txt"
                # Linux artifacts (may not exist if -NoLinux was used)
                "$ReleaseDir\huayu-$ver-$LinuxTriple.tar.gz"
            )
        }
        default {
            $files = @(
                "$ReleaseDir\$name-$ver-$Triple.zip"
                "$ReleaseDir\$name-version.txt"
                # Linux artifacts
                "$ReleaseDir\$name-$ver-$LinuxToolsTriple.tar.gz"
            )
        }
        "skills" {
            $files = @(
                "$ReleaseDir\skills-$ver-$Triple.zip"
                "$ReleaseDir\skills-version.txt"
                "$ReleaseDir\skills-$ver-$LinuxToolsTriple.tar.gz"
            )
        }
    }

    foreach ($f in $files) {
        if (-not (Test-Path $f)) {
            Write-Host "  [!] Missing: $f — skipping" -ForegroundColor Yellow
            continue
        }
        $fname = Split-Path $f -Leaf
        Step "scp $fname -> ${RemoteHost}:$RemotePath/"
        scp $f "${RemoteHost}:${RemotePath}/"
        if ($LASTEXITCODE -ne 0) {
            Fail "scp failed for $fname"
        }
    }
    Ok "$name $ver deployed"
}

Write-Host ""
Write-Host "  ─────────────────────────────────────────────────────" -ForegroundColor DarkGray
Write-Host "  $($toDeploy.Count) component(s) deployed: $($toDeploy -join ', ')" -ForegroundColor Green
Write-Host ""
