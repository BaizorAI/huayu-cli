# package-tools.ps1 鈥?Bundle codex + claude with portable Node.js for Windows
# Outputs zips to .\release\ then run deploy.sh to push to baizor.com
#
# Structure per tool zip:
#   node.exe                          <- portable Node.js runtime (no install required)
#   node_modules/.bin/{name}.cmd      <- launcher using relative node.exe path
#   node_modules/{package}/...        <- npm package files
#   {name}.version                    <- pinned version marker
#
# Usage:
#   .\package-tools.ps1
#
# Requires: Node.js / npm (for installing packages; node.exe in zip is separate)

param()

$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$Triple        = "x86_64-pc-windows-msvc"
$ReleaseDir    = "$PSScriptRoot\release"
$WorkDir       = [System.IO.Path]::Combine($env:TEMP, "huayu-tools-$([System.Guid]::NewGuid())")
# Read versions from versions.json (single source of truth)
$_versionsFile = "$PSScriptRoot\versions.json"
if (Test-Path $_versionsFile) {
    $_v = Get-Content $_versionsFile -Raw | ConvertFrom-Json
    $CodexVersion  = $_v.codex
    $ClaudeVersion = $_v.claude
} else {
    $CodexVersion  = "0.142.5"
    $ClaudeVersion = "1.0.3"
}

# Node.js LTS portable 鈥?this becomes node.exe bundled inside each tool zip
$NodeVersion   = "20.19.2"
$NodeZipUrl    = "https://nodejs.org/dist/v$NodeVersion/node-v$NodeVersion-win-x64.zip"

function Step([string]$msg) { Write-Host "  $msg" -ForegroundColor Cyan }
function Ok([string]$msg)   { Write-Host "  [ok] $msg" -ForegroundColor Green }
function Warn([string]$msg) { Write-Host "  [!]  $msg" -ForegroundColor Yellow }
function Fail([string]$msg) { Write-Host "`n  [error] $msg`n" -ForegroundColor Red; exit 1 }

Write-Host ""
Write-Host "  huayu tool bundler 鈥?Windows x64" -ForegroundColor White
Write-Host "  鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€" -ForegroundColor DarkGray

if (-not (Get-Command "npm" -ErrorAction SilentlyContinue)) {
    Fail "npm not found 鈥?install Node.js from https://nodejs.org"
}

New-Item -ItemType Directory -Path $WorkDir    -Force | Out-Null
New-Item -ItemType Directory -Path $ReleaseDir -Force | Out-Null

try {
    Set-Location $WorkDir

    # 鈹€鈹€ Download portable Node.js and extract node.exe 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
    Step "Downloading portable Node.js $NodeVersion ..."
    $nodeZipPath = "$WorkDir\node.zip"
    Invoke-WebRequest -Uri $NodeZipUrl -OutFile $nodeZipPath -UseBasicParsing
    $nodeExtract = "$WorkDir\node-extracted"
    Expand-Archive -Path $nodeZipPath -DestinationPath $nodeExtract -Force
    $nodeExe = Get-ChildItem -Path $nodeExtract -Filter "node.exe" -Recurse | Select-Object -First 1
    if (-not $nodeExe) { Fail "node.exe not found in Node.js archive" }
    $nodeExePath = $nodeExe.FullName
    Ok "node.exe  ($([Math]::Round((Get-Item $nodeExePath).Length / 1MB, 1)) MB)"

    # 鈹€鈹€ Helper: build one tool zip 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
    function Build-ToolZip {
        param(
            [string]$Name,
            [string]$Version,
            [string]$NpmPkg,       # e.g. "@openai/codex@0.142.5"
            [bool]$OmitOptional = $false
        )

        $pkgDir = "$WorkDir\pkg-$Name"
        New-Item -ItemType Directory $pkgDir -Force | Out-Null

        # --ignore-scripts: skip preinstall/postinstall (claude blocks Windows in preinstall)
        # --omit=optional: skip optional native deps 鈥?only for claude (sharp etc.)
        #                  codex NEEDS @openai/codex-win32-x64 (optional) for its native binary
        $npmArgs = @("install", "--prefix", $pkgDir, "--ignore-scripts")
        if ($OmitOptional) { $npmArgs += "--omit=optional" }
        $npmArgs += $NpmPkg
        Step "npm install $NpmPkg ..."
        & npm @npmArgs
        if ($LASTEXITCODE -ne 0) { Fail "npm install $NpmPkg failed (exit $LASTEXITCODE)" }

        # Resolve entry script from bin field
        $scope, $pkgName = $NpmPkg.Split("@", 2)[0], ($NpmPkg -replace "@[^@]+$","")
        # package.json is at node_modules/{pkg name without @scope prefix if any}
        $innerPkg = $NpmPkg -replace "@[^/]*/", "" -replace "@.*", ""
        # Try the scoped path first, then bare
        $pkgJsonCandidates = @(
            "$pkgDir\node_modules\$($NpmPkg -replace '@[^@]+$','' -replace '^@','@' )\package.json"
        ) + @(
            Get-ChildItem "$pkgDir\node_modules" -Filter "package.json" -Recurse -Depth 3 |
            Where-Object { $_.FullName -notmatch '[\\/]node_modules[\\/].*[\\/]node_modules' } |
            ForEach-Object { $_.FullName }
        ) | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1

        if (-not $pkgJsonCandidates) { Fail "package.json not found for $NpmPkg" }
        $pkgJson  = Get-Content $pkgJsonCandidates -Raw | ConvertFrom-Json
        $binEntry = ($pkgJson.bin.PSObject.Properties | Select-Object -First 1).Value
        # Make the entry path relative to node_modules root
        $pkgRelDir = (Split-Path $pkgJsonCandidates -Parent) -replace [regex]::Escape("$pkgDir\node_modules\"), ""
        $entryRelPath = "node_modules\$pkgRelDir\$binEntry" -replace "/", "\"

        # Create launcher .cmd 鈥?uses paths relative to its own location (%~dp0)
        # .cmd lives at node_modules\.bin\{name}.cmd
        # %~dp0 = tools\node_modules\.bin\   so ..\..\ = tools\
        $binDir = "$pkgDir\node_modules\.bin"
        New-Item -ItemType Directory $binDir -Force | Out-Null
        $launcher  = "@echo off`r`n"
        $launcher += "`"%~dp0..\..\node.exe`" `"%~dp0..\..\$entryRelPath`" %*`r`n"
        [System.IO.File]::WriteAllText("$binDir\$Name.cmd", $launcher)
        Ok "$Name.cmd launcher 鈫?node.exe + $entryRelPath"

        # Version marker
        [System.IO.File]::WriteAllText("$pkgDir\$Name.version", $Version)

        # Stage zip contents: node.exe + node_modules/ + {name}.version
        $stage = "$WorkDir\stage-$Name"
        New-Item -ItemType Directory $stage -Force | Out-Null
        Copy-Item $nodeExePath "$stage\node.exe" -Force
        Copy-Item "$pkgDir\node_modules" "$stage\node_modules" -Recurse -Force
        Copy-Item "$pkgDir\$Name.version" "$stage\$Name.version" -Force

        # Create zip
        $zipPath = "$ReleaseDir\$Name-$Version-$Triple.zip"
        if (Test-Path $zipPath) { Remove-Item $zipPath -Force }
        Compress-Archive -Path "$stage\*" -DestinationPath $zipPath -Force
        Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue

        # Version file for installer
        [System.IO.File]::WriteAllText("$ReleaseDir\$Name-version.txt", "$Version`n")
        Ok "$Name-$Version-$Triple.zip  ($([Math]::Round((Get-Item $zipPath).Length / 1MB, 1)) MB)"
    }

    # 鈹€鈹€ Build tool zips 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
    Build-ToolZip -Name "codex"  -Version $CodexVersion  -NpmPkg "@openai/codex@$CodexVersion"           -OmitOptional $false
    Build-ToolZip -Name "claude" -Version $ClaudeVersion -NpmPkg "@anthropic-ai/claude-code@$ClaudeVersion" -OmitOptional $true

} finally {
    Set-Location $PSScriptRoot
    Remove-Item -Recurse -Force $WorkDir -ErrorAction SilentlyContinue
}

Write-Host ""
Write-Host "  鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€" -ForegroundColor DarkGray
Write-Host "  release\ tool zips:" -ForegroundColor Green
Get-ChildItem "$ReleaseDir\codex-*.zip", "$ReleaseDir\claude-*.zip" -ErrorAction SilentlyContinue |
    ForEach-Object { Write-Host "    $($_.Name.PadRight(50)) $([Math]::Round($_.Length/1MB,1)) MB" }
Write-Host ""
Write-Host "  Next: " -NoNewline; Write-Host "./deploy.sh" -ForegroundColor White
Write-Host ""
