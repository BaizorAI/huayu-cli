#!/usr/bin/env bash
# package-linux.sh 鈥?Build huayu for Linux and bundle codex+claude as tar.gz
# Run on Linux (baizor server, WSL, or CI)
#
# Outputs to ./release/:
#   huayu-VERSION-TRIPLE.tar.gz
#   codex-CODEX_VERSION-TRIPLE.tar.gz
#   claude-CLAUDE_VERSION-TRIPLE.tar.gz
#   huayu-version.txt / codex-version.txt / claude-version.txt
#
# Usage:
#   bash package-linux.sh
#   bash package-linux.sh --skip-build        # skip cargo build
#   bash package-linux.sh --skip-tools        # skip node tool bundles

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RELEASE_DIR="$SCRIPT_DIR/release"
WORK_DIR=$(mktemp -d)
trap 'rm -rf "$WORK_DIR"' EXIT

# Read versions from versions.json (single source of truth)
VERSIONS_FILE="$SCRIPT_DIR/versions.json"
if [ -f "$VERSIONS_FILE" ] && command -v python3 &>/dev/null; then
    CODEX_VERSION=$(python3 -c "import json; print(json.load(open('$VERSIONS_FILE'))['codex'])")
    CLAUDE_VERSION=$(python3 -c "import json; print(json.load(open('$VERSIONS_FILE'))['claude'])")
elif [ -f "$VERSIONS_FILE" ] && command -v jq &>/dev/null; then
    CODEX_VERSION=$(jq -r '.codex' "$VERSIONS_FILE")
    CLAUDE_VERSION=$(jq -r '.claude' "$VERSIONS_FILE")
else
    CODEX_VERSION="0.142.5"
    CLAUDE_VERSION="1.0.3"
fi
NODE_VERSION="20.19.2"

SKIP_BUILD=false
SKIP_TOOLS=false
for arg in "$@"; do
    case "$arg" in
        --skip-build) SKIP_BUILD=true ;;
        --skip-tools) SKIP_TOOLS=true ;;
    esac
done

# 鈹€鈹€ helpers 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
step()  { echo "  $*"; }
ok()    { echo "  [ok] $*"; }
warn()  { echo "  [!]  $*"; }
fail()  { echo -e "\n  [error] $*\n" >&2; exit 1; }

need_cmd() { command -v "$1" &>/dev/null || fail "闇€瑕?$1 鈥?璇峰厛瀹夎"; }
need_cmd curl
need_cmd tar

echo ""
echo "  huayu Linux packager"
echo "  鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€"

mkdir -p "$RELEASE_DIR"

# 鈹€鈹€ detect arch 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
ARCH=$(uname -m)
case "$ARCH" in
    x86_64)          TRIPLE="x86_64-unknown-linux-gnu"    NODE_ARCH="x64"  ;;
    aarch64|arm64)   TRIPLE="aarch64-unknown-linux-gnu"   NODE_ARCH="arm64" ;;
    *) fail "涓嶆敮鎸佺殑鏋舵瀯: $ARCH" ;;
esac
step "鐩爣鏋舵瀯: $TRIPLE"

# 鈹€鈹€ cargo build 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
if [ "$SKIP_BUILD" = false ]; then
    need_cmd cargo
    step "cargo build --release ..."
    if command -v cross &>/dev/null; then
        cross build --release --target "$TRIPLE"
    else
        # Plain cargo 鈥?assumes musl target is installed:
        #   rustup target add x86_64-unknown-linux-musl
        cargo build --release --target "$TRIPLE"
    fi
    BINARY="$SCRIPT_DIR/target/$TRIPLE/release/huayu"
else
    # Try to find pre-built binary
    BINARY="$SCRIPT_DIR/target/$TRIPLE/release/huayu"
    [ -f "$BINARY" ] || fail "Binary not found at $BINARY 鈥?run without --skip-build first"
fi

VERSION=$("$BINARY" --version 2>&1 | awk '{print $NF}')
[ -n "$VERSION" ] || fail "鏃犳硶浠?binary 璇诲彇鐗堟湰"
ok "huayu $VERSION  ($TRIPLE)"

# 鈹€鈹€ bundle huayu binary 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
step "鎵撳寘 huayu-$VERSION-$TRIPLE.tar.gz ..."
HUAZHEN_STAGE="$WORK_DIR/huayu"
mkdir -p "$HUAZHEN_STAGE"
cp "$BINARY" "$HUAZHEN_STAGE/huayu"
chmod +x "$HUAZHEN_STAGE/huayu"
tar -czf "$RELEASE_DIR/huayu-$VERSION-$TRIPLE.tar.gz" -C "$HUAZHEN_STAGE" huayu
echo "$VERSION" > "$RELEASE_DIR/huayu-version.txt"
ok "huayu-$VERSION-$TRIPLE.tar.gz"

# 鈹€鈹€ portable Node.js 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
if [ "$SKIP_TOOLS" = false ]; then
    need_cmd npm

    NODE_TGZ="node-v$NODE_VERSION-linux-$NODE_ARCH.tar.gz"
    NODE_URL="https://nodejs.org/dist/v$NODE_VERSION/$NODE_TGZ"
    NODE_DIR="$WORK_DIR/nodejs"

    step "涓嬭浇 Node.js $NODE_VERSION ..."
    curl -fsSL "$NODE_URL" | tar -xz -C "$WORK_DIR"
    NODE_EXE=$(find "$WORK_DIR" -name "node" -type f | head -1)
    [ -f "$NODE_EXE" ] || fail "node binary not found in Node.js archive"
    ok "node  ($([[ -n "$NODE_EXE" ]] && du -sh "$NODE_EXE" | cut -f1))"

    # 鈹€鈹€ helper: bundle one tool 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
    build_tool_bundle() {
        local name="$1"
        local version="$2"
        local npm_pkg="$3"

        local pkg_dir="$WORK_DIR/pkg-$name"
        mkdir -p "$pkg_dir"

        step "npm install $npm_pkg ..."
        npm install --prefix "$pkg_dir" --ignore-scripts --omit=optional "${npm_pkg}@${version}"

        # Locate entry script via package.json bin field
        local pkg_json
        pkg_json=$(find "$pkg_dir/node_modules" -name "package.json" -maxdepth 3 \
            ! -path "*/node_modules/*/node_modules/*" | \
            xargs grep -l '"bin"' 2>/dev/null | head -1)
        [ -n "$pkg_json" ] || fail "package.json with bin field not found for $npm_pkg"

        local bin_rel
        bin_rel=$(node -e "const p=require('$pkg_json'); const b=p.bin; \
            const e=typeof b==='string'?b:Object.values(b)[0]; console.log(e)")

        local pkg_dir_rel
        pkg_dir_rel=$(dirname "$pkg_json" | sed "s|$pkg_dir/node_modules/||")

        local entry_path="node_modules/$pkg_dir_rel/$bin_rel"
        # Normalize double slashes
        entry_path=$(echo "$entry_path" | sed 's|//|/|g')

        # Create shell launcher at node_modules/.bin/{name}
        local bin_scripts="$pkg_dir/node_modules/.bin"
        mkdir -p "$bin_scripts"
        cat > "$bin_scripts/$name" <<EOF
#!/bin/sh
exec "\$(dirname "\$0")/../../node" "\$(dirname "\$0")/../../$entry_path" "\$@"
EOF
        chmod +x "$bin_scripts/$name"
        ok "$name launcher 鈫?node + $entry_path"

        # Version marker
        echo "$version" > "$pkg_dir/$name.version"

        # Stage: node + node_modules/ + {name}.version
        local stage="$WORK_DIR/stage-$name"
        mkdir -p "$stage"
        cp "$NODE_EXE" "$stage/node"
        chmod +x "$stage/node"
        cp -r "$pkg_dir/node_modules" "$stage/node_modules"
        cp "$pkg_dir/$name.version" "$stage/$name.version"

        # Create tar.gz
        local out="$RELEASE_DIR/$name-$version-$TRIPLE.tar.gz"
        tar -czf "$out" -C "$stage" .
        echo "$version" > "$RELEASE_DIR/$name-version.txt"
        ok "$name-$version-$TRIPLE.tar.gz  ($(du -sh "$out" | cut -f1))"
    }

    build_tool_bundle "codex"  "$CODEX_VERSION"  "@openai/codex"
    build_tool_bundle "claude" "$CLAUDE_VERSION" "@anthropic-ai/claude-code"
fi

# 鈹€鈹€ done 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€
echo ""
echo "  鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€"
echo "  release/ 鍐呭:"
ls -lh "$RELEASE_DIR"/*.tar.gz "$RELEASE_DIR"/*.txt 2>/dev/null | awk '{print "    "$NF, $5}'
echo ""
echo "  涓嬩竴姝? bash deploy.sh"
echo ""
