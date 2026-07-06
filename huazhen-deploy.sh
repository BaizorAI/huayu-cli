#!/usr/bin/env bash
# huazhen-deploy.sh — 一键编译并部署 huazhen (Windows + Linux)
#
# 用法:
#   bash huazhen-deploy.sh              # 完整编译 + 部署
#   bash huazhen-deploy.sh --skip-win   # 跳过 Windows 编译（重用已有 exe）
#   bash huazhen-deploy.sh --skip-linux # 跳过 Linux 编译
#   bash huazhen-deploy.sh --skip-build # 跳过所有编译（只重新打包+部署）
#
# 依赖:
#   - cargo (Rust)  — Windows 构建
#   - WSL Ubuntu    — Linux 交叉编译
#   - ssh/scp       — 部署到 baizor
#
# 当前工作目录须为 huazhen 源码根目录（含 Cargo.toml）

set -euo pipefail

# Load cargo into PATH (needed when running under Git Bash which doesn't
# inherit the Windows PATH entry added by the Rust installer).
source "$HOME/.cargo/env" 2>/dev/null || export PATH="$HOME/.cargo/bin:$PATH"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RELEASE_DIR="$SCRIPT_DIR/release"

# Convert Git Bash POSIX path (/f/foo/bar) to Windows path (F:\foo\bar).
# Uses only bash built-ins — avoids cygpath whose output varies by installation.
to_win_path() {
    local p="$1"
    if [[ "$p" =~ ^/([a-zA-Z])/(.*) ]]; then
        printf '%s' "${BASH_REMATCH[1]^^}:\\${BASH_REMATCH[2]}" | tr '/' '\\'
    else
        echo "$p"
    fi
}
WIN_SCRIPT_DIR=$(to_win_path "$SCRIPT_DIR")
WIN_RELEASE_DIR=$(to_win_path "$RELEASE_DIR")
WSL_SRC_WIN="C:\\wsl-build\\todo-app"
DEPLOY_HOST="baizor"
DEPLOY_PATH="/lucky/NewApi/data/install/"

SKIP_WIN=false
SKIP_LINUX=false
for arg in "$@"; do
    case "$arg" in
        --skip-win)   SKIP_WIN=true  ;;
        --skip-linux) SKIP_LINUX=true ;;
        --skip-build) SKIP_WIN=true; SKIP_LINUX=true ;;
    esac
done

step()  { echo -e "\n\033[1;34m── $* ──\033[0m"; }
ok()    { echo -e "  \033[1;32m[ok]\033[0m $*"; }
warn()  { echo -e "  \033[1;33m[!] \033[0m $*"; }
fail()  { echo -e "\n  \033[1;31m[error]\033[0m $*\n" >&2; exit 1; }

VERSION=$(grep '^version' "$SCRIPT_DIR/Cargo.toml" | head -1 | sed 's/.*= *"\(.*\)"/\1/')
echo ""
echo -e "  \033[1;37mhuazhen $VERSION — 编译 & 部署\033[0m"
echo "  ─────────────────────────────────────────────────────"

# ── 1. Windows 编译 ──────────────────────────────────────────────────────────

if [ "$SKIP_WIN" = false ]; then
    step "Windows 编译 (cargo build --release)"
    cd "$SCRIPT_DIR"
    cargo build --release || fail "cargo build 失败"
    ok "huazhen.exe"
else
    warn "跳过 Windows 编译 (--skip-win)"
fi

# ── 2. Windows 打包 ──────────────────────────────────────────────────────────

step "Windows 打包 (package.ps1 -SkipBuild)"
# Use -Command + & operator: avoids MSYS backslash mangling that breaks -File <path>.
powershell.exe -NonInteractive -Command "& { & '$WIN_SCRIPT_DIR\\package.ps1' -SkipBuild; exit \$LASTEXITCODE }" \
    || fail "package.ps1 失败"
ok "huazhen-x86_64-pc-windows-msvc.zip"

# ── 3. Linux 编译 ────────────────────────────────────────────────────────────

if [ "$SKIP_LINUX" = false ]; then
    step "同步源码到 WSL build 目录"
    powershell.exe -NonInteractive -Command "
        New-Item -ItemType Directory -Force -Path '${WSL_SRC_WIN}\\src' | Out-Null
        robocopy '${WIN_SCRIPT_DIR}\\src' '${WSL_SRC_WIN}\\src' /E /XD target /NFL /NDL /NJH /NJS | Out-Null
        Copy-Item '${WIN_SCRIPT_DIR}\\Cargo.toml' '${WSL_SRC_WIN}\\' -Force
        if (Test-Path '${WIN_SCRIPT_DIR}\\Cargo.lock') {
            Copy-Item '${WIN_SCRIPT_DIR}\\Cargo.lock' '${WSL_SRC_WIN}\\' -Force
        }
        Write-Host 'sync done'
    " || fail "源码同步失败"
    ok "源码已同步到 C:\\wsl-build\\todo-app"

    step "Linux 编译 (WSL cargo build --release)"
    wsl.exe -d Ubuntu -- bash -c \
        "cd /mnt/c/wsl-build/todo-app && ~/.cargo/bin/cargo build --release --target x86_64-unknown-linux-gnu 2>&1 | tail -5" \
        || fail "Linux 编译失败"
    ok "huazhen (x86_64-unknown-linux-gnu)"

    step "Linux 打包"
    # Write package.sh into WSL space so it can be called from there
    wsl.exe -d Ubuntu -- bash -c 'cat > /mnt/c/wsl-build/package.sh' <<'PKGEOF'
#!/usr/bin/env bash
set -euo pipefail
BINARY=/mnt/c/wsl-build/todo-app/target/x86_64-unknown-linux-gnu/release/huazhen
RELEASE=/mnt/c/wsl-build/todo-app/release
VERSION=$($BINARY --version 2>&1 | awk '{print $NF}')
mkdir -p "$RELEASE"
tar -czf "$RELEASE/huazhen-${VERSION}-x86_64-unknown-linux-gnu.tar.gz" -C "$(dirname "$BINARY")" huazhen
echo "done: huazhen-${VERSION}-x86_64-unknown-linux-gnu.tar.gz"
PKGEOF
    wsl.exe -d Ubuntu -- bash /mnt/c/wsl-build/package.sh || fail "Linux 打包失败"

    # Copy Linux tarball to release/
    powershell.exe -NonInteractive -Command "
        New-Item -ItemType Directory -Force -Path '$WIN_RELEASE_DIR' | Out-Null
        Copy-Item 'C:\\wsl-build\\todo-app\\release\\huazhen-${VERSION}-x86_64-unknown-linux-gnu.tar.gz' \
                  '$WIN_RELEASE_DIR\\' -Force
        Write-Host 'copied linux tarball'
    " || fail "Linux tarball 复制失败"
    ok "huazhen-${VERSION}-x86_64-unknown-linux-gnu.tar.gz"
else
    warn "跳过 Linux 编译 (--skip-linux)"
fi

# ── 4. 部署到 baizor ─────────────────────────────────────────────────────────

step "部署到 $DEPLOY_HOST:$DEPLOY_PATH"
scp \
    "$RELEASE_DIR/huazhen-x86_64-pc-windows-msvc.zip" \
    "$RELEASE_DIR/huazhen-${VERSION}-x86_64-unknown-linux-gnu.tar.gz" \
    "$RELEASE_DIR/huazhen-version.txt" \
    "$DEPLOY_HOST:$DEPLOY_PATH" \
    || fail "scp 失败"
ok "已部署到 $DEPLOY_HOST"

echo ""
echo -e "  \033[1;32m✓ 全部完成！huazhen $VERSION 已部署。\033[0m"
echo "  用户重启 TUI 或重新登录后即可使用新版本。"
echo ""
