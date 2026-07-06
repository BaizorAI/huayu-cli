#!/usr/bin/env bash
# huayu Linux installer
# Usage: curl -fsSL https://baizor.com/install/huayu.sh | bash

set -euo pipefail

BASE_URL="https://baizor.com/install"
huayu_HOME="${huayu_HOME:-$HOME/.huayu}"
BIN_DIR="$huayu_HOME/bin"
TOOLS_DIR="$huayu_HOME/tools"

# ── helpers ────────────────────────────────────────────────────────────────
step()  { echo "  $*"; }
ok()    { echo "  [ok] $*"; }
warn()  { echo "  [!]  $*"; }
fail()  { echo -e "\n  [error] $*\n" >&2; exit 1; }

need_cmd() { command -v "$1" &>/dev/null || fail "需要 $1 — 请先安装"; }
need_cmd curl
need_cmd tar

# ── arch ───────────────────────────────────────────────────────────────────
ARCH=$(uname -m)
case "$ARCH" in
    x86_64)         TRIPLE="x86_64-unknown-linux-gnu" ;;
    aarch64|arm64)  TRIPLE="aarch64-unknown-linux-gnu" ;;
    *) fail "不支持的架构: $ARCH" ;;
esac

# ── version ────────────────────────────────────────────────────────────────
echo ""
echo "huayu installer"
echo "─────────────────────────────────────────────────────────"
step "正在获取最新版本 ..."
VERSION=$(curl -fsSL "$BASE_URL/huayu-version.txt" | tr -d '[:space:]')
[ -n "$VERSION" ] || fail "无法从 baizor.com 获取版本信息"
ok "最新版本: $VERSION"

# ── huayu binary ─────────────────────────────────────────────────────────
mkdir -p "$BIN_DIR" "$TOOLS_DIR"

step "下载 huayu-$VERSION-$TRIPLE.tar.gz ..."
curl -fsSL "$BASE_URL/huayu-$VERSION-$TRIPLE.tar.gz" | tar -xz -C "$BIN_DIR"
chmod +x "$BIN_DIR/huayu"
ok "huayu  ->  $BIN_DIR"

# ── tools: codex + claude ──────────────────────────────────────────────────
install_tool() {
    local name="$1"
    local version="$2"
    local zip="$name-$version-$TRIPLE.tar.gz"
    step "下载 $zip ..."
    if curl -fsSL "$BASE_URL/$zip" | tar -xz -C "$TOOLS_DIR"; then
        # Ensure launcher script is executable
        local launcher="$TOOLS_DIR/node_modules/.bin/$name"
        [ -f "$launcher" ] && chmod +x "$launcher"
        ok "$name $version"
    else
        warn "$name 下载失败 — 启动后运行 'huayu update $name' 重试"
    fi
}

# Fetch pinned tool versions from version files if available
CODEX_VERSION=$(curl -fsSL "$BASE_URL/codex-version.txt" 2>/dev/null | tr -d '[:space:]' || echo "0.142.5")
CLAUDE_VERSION=$(curl -fsSL "$BASE_URL/claude-version.txt" 2>/dev/null | tr -d '[:space:]' || echo "1.0.3")

install_tool "codex"  "$CODEX_VERSION"
install_tool "claude" "$CLAUDE_VERSION"

# ── PATH ───────────────────────────────────────────────────────────────────
add_to_path() {
    local rc_file="$1"
    local line='export PATH="$HOME/.huayu/bin:$PATH"'
    if [ -f "$rc_file" ] && ! grep -qF '.huayu/bin' "$rc_file"; then
        echo "" >> "$rc_file"
        echo "# huayu" >> "$rc_file"
        echo "$line" >> "$rc_file"
        ok "已添加 PATH → $rc_file"
    fi
}

add_to_path "$HOME/.bashrc"
add_to_path "$HOME/.zshrc"
[ -f "$HOME/.profile" ] && add_to_path "$HOME/.profile"

export PATH="$BIN_DIR:$PATH"

# ── codex model config ─────────────────────────────────────────────────────
mkdir -p "$HOME/.codex"
if [ ! -f "$HOME/.codex/config.toml" ]; then
    cat > "$HOME/.codex/config.toml" << 'CODEX_CONFIG'
[model_info."huayu-v1"]
context_window = 128000
max_output_tokens = 16384

[model_info."huayu-fable-5"]
context_window = 128000
max_output_tokens = 16384

[model_info."huayu3.6-35b"]
context_window = 32768
max_output_tokens = 8192
CODEX_CONFIG
    ok "已创建 ~/.codex/config.toml"
fi

# ── done ───────────────────────────────────────────────────────────────────
echo ""
echo "─────────────────────────────────────────────────────────"
echo "  huayu $VERSION 安装完成！"
echo ""
echo "  启动:   huayu"
echo "  登录:   huayu login"
echo ""
echo "  如果找不到 huayu 命令，请重新打开终端或运行:"
echo "    source ~/.bashrc"
echo ""
