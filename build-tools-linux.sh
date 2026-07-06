#!/usr/bin/env bash
# Build codex + claude Linux bundles on baizor, output to /lucky/NewApi/data/install/
# Usage: bash /tmp/build-tools-linux.sh

set -euo pipefail

INSTALL_DIR="/lucky/NewApi/data/install"
WORK_DIR=$(mktemp -d)
trap 'rm -rf "$WORK_DIR"' EXIT

NODE_VERSION="20.19.2"
CODEX_VERSION="0.142.5"
CLAUDE_VERSION="1.0.3"
TRIPLE="x86_64-unknown-linux-gnu"

step()  { echo "  $*"; }
ok()    { echo "  [ok] $*"; }
warn()  { echo "  [!]  $*"; }
fail()  { echo "  [error] $*" >&2; exit 1; }

echo ""
echo "  huayu Linux tool bundler"
echo "  ─────────────────────────────────────────────────────"

# ── Download portable Node.js ──────────────────────────────────────────────
step "Downloading Node.js $NODE_VERSION ..."
NODE_TAR="node-v$NODE_VERSION-linux-x64.tar.gz"
NODE_DIR="$WORK_DIR/nodejs"
mkdir -p "$NODE_DIR"
curl -fsSL "https://nodejs.org/dist/v$NODE_VERSION/$NODE_TAR" | tar -xz --strip-components=1 -C "$NODE_DIR"
NODE_BIN="$NODE_DIR/bin/node"
NPM_BIN="$NODE_DIR/bin/npm"
chmod +x "$NODE_BIN"
# Add node to PATH so npm's shebang (#!/usr/bin/env node) resolves correctly
export PATH="$NODE_DIR/bin:$PATH"
ok "node $("$NODE_BIN" --version)  npm $("$NPM_BIN" --version)"

# ── Build one tool bundle ──────────────────────────────────────────────────
build_bundle() {
    local name="$1"
    local version="$2"
    local npm_pkg="$3"

    local pkg_dir="$WORK_DIR/pkg-$name"
    mkdir -p "$pkg_dir"

    step "npm install ${npm_pkg}@${version} ..."
    local npm_opts="--prefix $pkg_dir --ignore-scripts"
    # codex requires its native optional dep (@openai/codex-linux-x64)
    [ "$name" != "codex" ] && npm_opts="$npm_opts --omit=optional"
    "$NPM_BIN" install $npm_opts "${npm_pkg}@${version}"

    # Resolve entry from bin field
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
    # normalize double slashes
    entry_path=$(echo "$entry_path" | tr -s '/')

    # Create launcher shell script at node_modules/.bin/{name}
    # npm creates a symlink here — remove it first so cat > doesn't follow it into the JS file
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

    # Create tar.gz
    local out="$INSTALL_DIR/$name-$version-$TRIPLE.tar.gz"
    tar -czf "$out" -C "$stage" .
    echo "$version" > "$INSTALL_DIR/$name-version.txt"
    local size
    size=$(du -sh "$out" | cut -f1)
    ok "$name-$version-$TRIPLE.tar.gz  ($size)"
}

mkdir -p "$INSTALL_DIR"
build_bundle "codex"  "$CODEX_VERSION"  "@openai/codex"
build_bundle "claude" "$CLAUDE_VERSION" "@anthropic-ai/claude-code"

echo ""
echo "  ─────────────────────────────────────────────────────"
echo "  完成！"
ls -lh "$INSTALL_DIR"/*.tar.gz "$INSTALL_DIR"/*-version.txt 2>/dev/null | awk '{print "    " $NF, $5}'
echo ""
