# build.ps1 — Smart build for huayu / codex / claude
# Only builds components that have changed since the last build.
# Auto-bumps patch version and syncs to all files.
#
# Usage:
#   .\build.ps1                     # build only changed components
#   .\build.ps1 -Force              # force-build all
#   .\build.ps1 -Component huayu    # only check/build huayu
#   .\build.ps1 -Component codex    # only check/build codex
#   .\build.ps1 -NoBump             # build without version bump

param(
    [switch]$Force,
    [switch]$NoBump,
    [ValidateSet("huayu", "codex", "claude")]
    [string]$Component
)

$ErrorActionPreference = 'Stop'
$ScriptDir = $PSScriptRoot

# ── Helpers ────────────────────────────────────────────────────────────────

function Step([string]$msg)  { Write-Host "  $msg" -ForegroundColor Cyan }
function Ok([string]$msg)    { Write-Host "  [ok] $msg" -ForegroundColor Green }
function Skip([string]$msg)  { Write-Host "  [skip] $msg" -ForegroundColor DarkGray }
function Build([string]$msg) { Write-Host "  [build] $msg" -ForegroundColor Yellow }
function Fail([string]$msg)  { Write-Host "`n  [error] $msg`n" -ForegroundColor Red; exit 1 }

# ── Read versions.json ─────────────────────────────────────────────────────

$VersionsFile = "$ScriptDir\versions.json"
if (-not (Test-Path $VersionsFile)) {
    Fail "versions.json not found at $VersionsFile"
}
$Versions = Get-Content $VersionsFile -Raw | ConvertFrom-Json

# ── Read .build-state.json ─────────────────────────────────────────────────

$StateFile = "$ScriptDir\.build-state.json"
if (Test-Path $StateFile) {
    $State = Get-Content $StateFile -Raw | ConvertFrom-Json
} else {
    $State = [PSCustomObject]@{}
}

# ── Fingerprint functions ──────────────────────────────────────────────────

function Get-HuayuSourceHash {
    $files = @()
    $files += Get-ChildItem "$ScriptDir\src" -Recurse -Include "*.rs" | Sort-Object FullName
    $files += Get-Item "$ScriptDir\Cargo.toml"
    if (Test-Path "$ScriptDir\Cargo.lock") {
        $files += Get-Item "$ScriptDir\Cargo.lock"
    }

    $stream = [System.IO.MemoryStream]::new()
    foreach ($f in $files) {
        $bytes = [System.IO.File]::ReadAllBytes($f.FullName)
        $stream.Write($bytes, 0, $bytes.Length)
    }
    $stream.Position = 0
    return (Get-FileHash -InputStream $stream -Algorithm SHA256).Hash
}

# ── Version bump ───────────────────────────────────────────────────────────

function Bump-PatchVersion([string]$ver) {
    $parts = $ver.Split(".")
    if ($parts.Length -eq 3) {
        $parts[2] = [string]([int]$parts[2] + 1)
        return $parts -join "."
    }
    return "$ver.1"
}

# ── Sync versions to all files ─────────────────────────────────────────────

function Sync-AllVersions {
    param(
        [string]$HuayuVer,
        [string]$CodexVer,
        [string]$ClaudeVer
    )

    # 1. Cargo.toml (UTF-8 safe)
    $cargoPath = "$ScriptDir\Cargo.toml"
    $utf8NoBom = [System.Text.UTF8Encoding]::new($false)
    $cargo = [System.IO.File]::ReadAllText($cargoPath, $utf8NoBom)
    $cargo = $cargo -replace '(?m)^(version\s*=\s*")[^"]+(")', "`${1}$HuayuVer`${2}"
    [System.IO.File]::WriteAllText($cargoPath, $cargo, $utf8NoBom)

    # 2. installer.rs (UTF-8 — contains Chinese characters, must preserve encoding)
    $installerPath = "$ScriptDir\src\services\installer.rs"
    if (Test-Path $installerPath) {
        $utf8NoBom = [System.Text.UTF8Encoding]::new($false)
        $rs = [System.IO.File]::ReadAllText($installerPath, $utf8NoBom)
        $rs = $rs -replace '(CODEX_VERSION:\s*&str\s*=\s*")[^"]+(")', "`${1}$CodexVer`${2}"
        $rs = $rs -replace '(CLAUDE_VERSION:\s*&str\s*=\s*")[^"]+(")', "`${1}$ClaudeVer`${2}"
        [System.IO.File]::WriteAllText($installerPath, $rs, $utf8NoBom)
    }

    # 3. package-tools.ps1
    $ptPath = "$ScriptDir\package-tools.ps1"
    if (Test-Path $ptPath) {
        $utf8NoBom = [System.Text.UTF8Encoding]::new($false)
        $pt = [System.IO.File]::ReadAllText($ptPath, $utf8NoBom)
        $pt = $pt -replace '(\$CodexVersion\s*=\s*")[^"]+(")', "`${1}$CodexVer`${2}"
        $pt = $pt -replace '(\$ClaudeVersion\s*=\s*")[^"]+(")', "`${1}$ClaudeVer`${2}"
        [System.IO.File]::WriteAllText($ptPath, $pt, $utf8NoBom)
    }

    # 4. package-linux.sh (UTF-8 — contains Chinese characters)
    $plPath = "$ScriptDir\package-linux.sh"
    if (Test-Path $plPath) {
        $utf8NoBom = [System.Text.UTF8Encoding]::new($false)
        $pl = [System.IO.File]::ReadAllText($plPath, $utf8NoBom)
        $pl = $pl -replace '(CODEX_VERSION=")[^"]+(")', "`${1}$CodexVer`${2}"
        $pl = $pl -replace '(CLAUDE_VERSION=")[^"]+(")', "`${1}$ClaudeVer`${2}"
        [System.IO.File]::WriteAllText($plPath, $pl, $utf8NoBom)
    }
}

# ── Save state ─────────────────────────────────────────────────────────────

function Save-BuildState {
    $json = $State | ConvertTo-Json -Depth 5
    [System.IO.File]::WriteAllText($StateFile, $json)
}

function Save-Versions {
    $json = $Versions | ConvertTo-Json -Depth 5
    [System.IO.File]::WriteAllText($VersionsFile, $json)
}

# ── Build logic per component ──────────────────────────────────────────────

function Build-Huayu {
    $currentHash = Get-HuayuSourceHash
    $lastHash = if ($State.PSObject.Properties["huayu"]) { $State.huayu.hash } else { "" }
    $currentVer = $Versions.huayu

    if (-not $Force -and ($currentHash -eq $lastHash)) {
        Skip "huayu $currentVer — no changes"
        return $false
    }

    # Bump version
    $newVer = $currentVer
    if (-not $NoBump -and ($currentHash -ne $lastHash) -and $lastHash -ne "") {
        $newVer = Bump-PatchVersion $currentVer
        $Versions.huayu = $newVer
        Build "huayu $currentVer -> $newVer (source changed)"
    } else {
        Build "huayu $newVer"
    }

    # Sync versions & build
    Sync-AllVersions -HuayuVer $Versions.huayu -CodexVer $Versions.codex -ClaudeVer $Versions.claude
    Save-Versions

    Step "cargo build --release ..."
    Push-Location $ScriptDir
    cargo build --release
    $rc = $LASTEXITCODE
    Pop-Location
    if ($rc -ne 0) { Fail "cargo build failed (exit $rc)" }

    # Package
    Step "Packaging ..."
    & "$ScriptDir\package.ps1" -SkipBuild
    Ok "huayu $newVer built and packaged"

    # Install to local .huayu
    $exeSrc = "$ScriptDir\target\release\huayu.exe"
    $exeDst = "$env:USERPROFILE\.huayu\bin\huayu.exe"
    if (Test-Path $exeSrc) {
        try {
            Copy-Item $exeSrc $exeDst -Force -ErrorAction Stop
            Ok "Installed to $exeDst"
        } catch {
            Write-Host "  [!] Could not copy to $exeDst (may be in use)" -ForegroundColor Yellow
        }
    }

    # Update state — recompute hash AFTER Sync-AllVersions, because it modifies
    # Cargo.toml and installer.rs (which are part of the source hash). Saving
    # the post-sync hash prevents false "source changed" on the next run.
    $postSyncHash = Get-HuayuSourceHash
    $State | Add-Member -NotePropertyName "huayu" -NotePropertyValue ([PSCustomObject]@{
        hash    = $postSyncHash
        version = $newVer
    }) -Force
    Save-BuildState
    return $true
}

function Build-Tool([string]$Name) {
    $currentVer = $Versions.$Name
    $lastVer = if ($State.PSObject.Properties[$Name]) { $State.$Name.version } else { "" }

    if (-not $Force -and ($currentVer -eq $lastVer)) {
        Skip "$Name $currentVer — no changes"
        return $false
    }

    if ($lastVer -and ($lastVer -ne $currentVer)) {
        Build "$Name $lastVer -> $currentVer (version changed in versions.json)"
    } else {
        Build "$Name $currentVer"
    }

    # Sync versions to files that embed them
    Sync-AllVersions -HuayuVer $Versions.huayu -CodexVer $Versions.codex -ClaudeVer $Versions.claude

    # Build the specific tool using package-tools.ps1 logic
    Step "Building $Name $currentVer ..."
    $Triple = "x86_64-pc-windows-msvc"
    $ReleaseDir = "$ScriptDir\release"
    New-Item -ItemType Directory -Path $ReleaseDir -Force | Out-Null

    # Determine npm package name
    $npmPkg = switch ($Name) {
        "codex"  { "@openai/codex@$currentVer" }
        "claude" { "@anthropic-ai/claude-code@$currentVer" }
    }
    $omitOptional = ($Name -eq "claude")

    # Invoke package-tools.ps1 (it handles npm install + bundling)
    & "$ScriptDir\package-tools.ps1"
    Ok "$Name $currentVer built and packaged"

    # Update state
    $State | Add-Member -NotePropertyName $Name -NotePropertyValue ([PSCustomObject]@{
        hash    = $currentVer
        version = $currentVer
    }) -Force
    Save-BuildState
    return $true
}

# ── Main ───────────────────────────────────────────────────────────────────

Write-Host ""
Write-Host "  huayu build system" -ForegroundColor White
Write-Host "  ─────────────────────────────────────────────────────" -ForegroundColor DarkGray
Write-Host "  versions.json: huayu=$($Versions.huayu) codex=$($Versions.codex) claude=$($Versions.claude)" -ForegroundColor DarkGray
Write-Host ""

$built = 0

$components = if ($Component) { @($Component) } else { @("huayu", "codex", "claude") }

foreach ($c in $components) {
    switch ($c) {
        "huayu" {
            if (Build-Huayu) { $built++ }
        }
        default {
            if (Build-Tool $c) { $built++ }
        }
    }
}

Write-Host ""
Write-Host "  ─────────────────────────────────────────────────────" -ForegroundColor DarkGray
if ($built -eq 0) {
    Write-Host "  No changes detected. Nothing to build." -ForegroundColor DarkGray
} else {
    Write-Host "  $built component(s) built." -ForegroundColor Green
    Write-Host "  Next: " -NoNewline; Write-Host ".\deploy.ps1" -ForegroundColor White
}
Write-Host ""
