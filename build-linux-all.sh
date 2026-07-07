#!/usr/bin/env bash
# build-linux-all.sh — Build huayu + codex + claude for Linux in WSL
#
# Combines musl cross-compilation (from build-linux.sh) with Node.js tool
# bundling (from build-tools-linux.sh / package-linux.sh) into a single
# script that outputs all artifacts to ./release/.
#
# Usage:
#   wsl bash build-linux-all.sh                # full build
#   wsl bash build-linux-all.sh --skip-build   # skip cargo build (reuse existing binary)
#   wsl bash build-linux-all.sh --skip-tools   # skip codex/claude bundling

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RELEASE_DIR="$SCRIPT_DIR/release"
WORK_DIR=$(mktemp -d)
trap 'rm -rf "$WORK_DIR"' EXIT

# ── Parse args ────────────────────────────────────────────────────────────
SKIP_BUILD=false
SKIP_TOOLS=false
for arg in "$@"; do
    case "$arg" in
        --skip-build) SKIP_BUILD=true ;;
        --skip-tools) SKIP_TOOLS=true ;;
    esac
done

# ── Read versions from versions.json ──────────────────────────────────────
VERSIONS_FILE="$SCRIPT_DIR/versions.json"
if [ ! -f "$VERSIONS_FILE" ]; then
    echo "  [error] versions.json not found" >&2
    exit 1
fi

# Parse with python3 (usually available in WSL), fall back to jq, then grep
read_version() {
    local key="$1"
    if command -v python3 &>/dev/null; then
        python3 -c "import json; print(json.load(open('$VERSIONS_FILE'))['$key'])"
    elif command -v jq &>/dev/null; then
        jq -r ".$key" "$VERSIONS_FILE"
    else
        grep -oP "\"$key\"\\s*:\\s*\"\\K[^\"]*" "$VERSIONS_FILE"
    fi
}

CODEX_VERSION=$(read_version codex)
CLAUDE_VERSION=$(read_version claude)
NODE_VERSION="20.19.2"
TRIPLE="x86_64-unknown-linux-musl"
TOOLS_TRIPLE="x86_64-unknown-linux-gnu"

# ── Helpers ───────────────────────────────────────────────────────────────
step()  { echo "  $*"; }
ok()    { echo "  [ok] $*"; }
warn()  { echo "  [!]  $*"; }
fail()  { echo -e "\n  [error] $*\n" >&2; exit 1; }

echo ""
echo "  huayu Linux builder (WSL)"
echo "  ─────────────────────────────────────────────────────"
echo "  codex=$CODEX_VERSION  claude=$CLAUDE_VERSION  node=$NODE_VERSION"
echo ""

mkdir -p "$RELEASE_DIR"

# Load cargo/rustup if available
source "$HOME/.cargo/env" 2>/dev/null || true

# ══════════════════════════════════════════════════════════════════════════
# Phase 1: Build huayu binary (musl static linking)
# ══════════════════════════════════════════════════════════════════════════

if [ "$SKIP_BUILD" = false ]; then
    # Ensure musl cross-compiler is installed
    step "Setting up musl cross-compiler ..."
    MUSL_DIR="$HOME/.musl-cross"
    if [ ! -f "$MUSL_DIR/bin/x86_64-linux-musl-gcc" ]; then
        mkdir -p "$MUSL_DIR"
        curl -fsSL https://musl.cc/x86_64-linux-musl-cross.tgz \
            | tar -xz --strip-components=1 -C "$MUSL_DIR"
        ok "musl-gcc installed to $MUSL_DIR"
    else
        ok "musl-gcc already present"
    fi

    export CC_x86_64_unknown_linux_musl="$MUSL_DIR/bin/x86_64-linux-musl-gcc"
    export CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER="$MUSL_DIR/bin/x86_64-linux-musl-gcc"

    # Ensure the musl target is available
    rustup target add x86_64-unknown-linux-musl 2>/dev/null || true

    step "cargo build --release --target $TRIPLE ..."
    cd "$SCRIPT_DIR"
    cargo build --release --target "$TRIPLE"

    BINARY="$SCRIPT_DIR/target/$TRIPLE/release/huayu"
    VERSION=$("$BINARY" --version 2>&1 | awk '{print $NF}')
    ok "huayu $VERSION ($TRIPLE)"

    step "Packaging huayu-$VERSION-$TRIPLE.tar.gz ..."
    STAGE="$WORK_DIR/huayu-stage"
    mkdir -p "$STAGE"
    cp "$BINARY" "$STAGE/huayu"
    chmod +x "$STAGE/huayu"
    tar -czf "$RELEASE_DIR/huayu-$VERSION-$TRIPLE.tar.gz" -C "$STAGE" huayu
    echo "$VERSION" > "$RELEASE_DIR/huayu-version.txt"
    ok "huayu-$VERSION-$TRIPLE.tar.gz"
else
    BINARY="$SCRIPT_DIR/target/$TRIPLE/release/huayu"
    if [ -f "$BINARY" ]; then
        VERSION=$("$BINARY" --version 2>&1 | awk '{print $NF}')
        ok "Using existing binary: huayu $VERSION ($TRIPLE)"
    else
        warn "Binary not found at $BINARY — huayu tar.gz will not be created"
    fi
fi

# ══════════════════════════════════════════════════════════════════════════
# Phase 2: Bundle codex + claude with portable Node.js
# ══════════════════════════════════════════════════════════════════════════

if [ "$SKIP_TOOLS" = false ]; then
    # Download portable Node.js for Linux x64
    step "Downloading Node.js $NODE_VERSION ..."
    NODE_TAR="node-v$NODE_VERSION-linux-x64.tar.gz"
    NODE_DIR="$WORK_DIR/nodejs"
    mkdir -p "$NODE_DIR"
    curl -fsSL "https://nodejs.org/dist/v$NODE_VERSION/$NODE_TAR" \
        | tar -xz --strip-components=1 -C "$NODE_DIR"
    NODE_BIN="$NODE_DIR/bin/node"
    NPM_BIN="$NODE_DIR/bin/npm"
    chmod +x "$NODE_BIN"
    export PATH="$NODE_DIR/bin:$PATH"
    ok "node $("$NODE_BIN" --version)  npm $("$NPM_BIN" --version)"

    # ── Build one tool bundle ─────────────────────────────────────────────
    build_tool_bundle() {
        local name="$1"
        local version="$2"
        local npm_pkg="$3"

        local pkg_dir="$WORK_DIR/pkg-$name"
        mkdir -p "$pkg_dir"

        step "npm install ${npm_pkg}@${version} ..."
        local npm_opts="--prefix $pkg_dir --ignore-scripts"
        # codex needs its native optional dep (@openai/codex-linux-x64)
        [ "$name" != "codex" ] && npm_opts="$npm_opts --omit=optional"
        "$NPM_BIN" install $npm_opts "${npm_pkg}@${version}"

        # Resolve entry from bin field in package.json
        local pkg_json
        pkg_json=$(find "$pkg_dir/node_modules" -name "package.json" -maxdepth 3 \
            ! -path "*/node_modules/*/node_modules/*" | \
            xargs grep -l '"bin"' 2>/dev/null | head -1)
        [ -n "$pkg_json" ] || fail "package.json with bin not found for $npm_pkg"

        local bin_rel
        bin_rel=$("$NODE_BIN" -e "
const p = require('$pkg_json');
const b = p.bin;
const e = typeof b === 'string' ? b : Object.values(b)[0];
process.stdout.write(e);
")
        local pkg_rel
        pkg_rel=$(dirname "$pkg_json" | sed "s|$pkg_dir/node_modules/||")
        local entry_path="node_modules/$pkg_rel/$bin_rel"
        entry_path=$(echo "$entry_path" | tr -s '/')

        # Create shell launcher at node_modules/.bin/{name}
        local bin_scripts="$pkg_dir/node_modules/.bin"
        mkdir -p "$bin_scripts"
        rm -f "$bin_scripts/$name"
        cat > "$bin_scripts/$name" <<LAUNCHER
#!/bin/sh
SELF=\$(cd "\$(dirname "\$0")" && pwd)
exec "\$SELF/../../node" "\$SELF/../../$entry_path" "\$@"
LAUNCHER
        chmod +x "$bin_scripts/$name"
        ok "$name launcher → node + $entry_path"

        # Version marker
        echo "$version" > "$pkg_dir/$name.version"

        # Stage: node + node_modules/ + {name}.version
        local stage="$WORK_DIR/stage-$name"
        mkdir -p "$stage"
        cp "$NODE_BIN" "$stage/node"
        chmod +x "$stage/node"
        cp -r "$pkg_dir/node_modules" "$stage/node_modules"
        cp "$pkg_dir/$name.version" "$stage/$name.version"

        # Create tar.gz (use TOOLS_TRIPLE — tools run on gnu, not musl)
        local out="$RELEASE_DIR/$name-$version-$TOOLS_TRIPLE.tar.gz"
        tar -czf "$out" -C "$stage" .
        echo "$version" > "$RELEASE_DIR/$name-version.txt"
        local size
        size=$(du -sh "$out" | cut -f1)
        ok "$name-$version-$TOOLS_TRIPLE.tar.gz  ($size)"
    }

    build_tool_bundle "codex"  "$CODEX_VERSION"  "@openai/codex"
    build_tool_bundle "claude" "$CLAUDE_VERSION"  "@anthropic-ai/claude-code"
fi

# ── Summary ───────────────────────────────────────────────────────────────
echo ""
echo "  ─────────────────────────────────────────────────────"
echo "  release/ Linux 产物:"
ls -lh "$RELEASE_DIR"/*.tar.gz 2>/dev/null | awk '{print "    " $NF, $5}' || echo "    (none)"
echo ""
