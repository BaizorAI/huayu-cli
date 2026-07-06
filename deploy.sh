#!/usr/bin/env bash
# Deploy release assets to baizor.com
# Usage: ./deploy.sh
#
# Requires: ./release/ populated by package.ps1 / package-linux.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RELEASE_DIR="$SCRIPT_DIR/release"

if [ ! -d "$RELEASE_DIR" ] || [ -z "$(ls -A "$RELEASE_DIR" 2>/dev/null)" ]; then
    echo "  [error] release/ is empty — run package.ps1 or package-linux.sh first"
    exit 1
fi

echo "→ Deploying release/ to baizor:/lucky/NewApi/data/install/ ..."
scp "$RELEASE_DIR"/* baizor:/lucky/NewApi/data/install/
echo "✓ Done"
