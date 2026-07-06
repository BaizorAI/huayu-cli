#!/usr/bin/env bash
# Build huayu for Linux musl in WSL
# Usage: wsl bash build-linux.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RELEASE_DIR="$SCRIPT_DIR/release"

source "$HOME/.cargo/env" 2>/dev/null || true

echo "  [1/4] Setting up musl cross-compiler ..."
MUSL_DIR="$HOME/.musl-cross"
if [ ! -f "$MUSL_DIR/bin/x86_64-linux-musl-gcc" ]; then
    mkdir -p "$MUSL_DIR"
    curl -fsSL https://musl.cc/x86_64-linux-musl-cross.tgz | tar -xz --strip-components=1 -C "$MUSL_DIR"
    echo "  [ok] musl-gcc installed to $MUSL_DIR"
else
    echo "  [ok] musl-gcc already present"
fi

export CC_x86_64_unknown_linux_musl="$MUSL_DIR/bin/x86_64-linux-musl-gcc"
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER="$MUSL_DIR/bin/x86_64-linux-musl-gcc"

echo "  [2/4] cargo build --release --target x86_64-unknown-linux-musl ..."
cd "$SCRIPT_DIR"
cargo build --release --target x86_64-unknown-linux-musl

BINARY="$SCRIPT_DIR/target/x86_64-unknown-linux-musl/release/huayu"
VERSION=$("$BINARY" --version 2>&1 | awk '{print $NF}')
TRIPLE="x86_64-unknown-linux-musl"

echo "  [ok] huayu $VERSION ($TRIPLE)"

echo "  [3/4] Packaging tar.gz ..."
mkdir -p "$RELEASE_DIR"
TMPDIR=$(mktemp -d)
cp "$BINARY" "$TMPDIR/huayu"
chmod +x "$TMPDIR/huayu"
tar -czf "$RELEASE_DIR/huayu-$VERSION-$TRIPLE.tar.gz" -C "$TMPDIR" huayu
rm -rf "$TMPDIR"
echo "$VERSION" > "$RELEASE_DIR/huayu-version.txt"
echo "  [ok] huayu-$VERSION-$TRIPLE.tar.gz"

echo ""
echo "  [4/4] Deploying to baizor ..."
scp "$RELEASE_DIR/huayu-$VERSION-$TRIPLE.tar.gz" \
    "$RELEASE_DIR/huayu-version.txt" \
    baizor:/lucky/NewApi/data/install/
echo "  [ok] Deployed!"
echo ""
echo "  Test: curl -fsSL https://baizor.com/install/huayu.sh | bash"
