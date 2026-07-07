#!/usr/bin/env bash
# deploy.sh — Smart deploy: only scp components with version changes
# Compares local .build-state.json vs remote *-version.txt on baizor.
#
# Usage:
#   bash deploy.sh              # deploy changed components
#   bash deploy.sh --dry-run    # show diff only
#   bash deploy.sh --force      # deploy all regardless of version

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RELEASE_DIR="$SCRIPT_DIR/release"
STATE_FILE="$SCRIPT_DIR/.build-state.json"
REMOTE_HOST="baizor"
REMOTE_PATH="/lucky/NewApi/data/install"

DRY_RUN=false
FORCE=false
for arg in "$@"; do
    case "$arg" in
        --dry-run) DRY_RUN=true ;;
        --force)   FORCE=true ;;
    esac
done

# ── helpers ──────────────────────────────────────────────────────────────
ok()     { echo "  [ok] $*"; }
skip()   { echo "  [skip] $*"; }
deploy() { echo "  [deploy] $*"; }
step()   { echo "  $*"; }
fail()   { echo -e "\n  [error] $*\n" >&2; exit 1; }

# ── read local versions from .build-state.json ──────────────────────────
[ -f "$STATE_FILE" ] || fail ".build-state.json not found — run build.ps1 first"

local_version() {
    local name="$1"
    if command -v python3 &>/dev/null; then
        python3 -c "
import json, sys
d = json.load(open('$STATE_FILE'))
print(d.get('$name',{}).get('version',''))" 2>/dev/null
    elif command -v jq &>/dev/null; then
        jq -r ".[\"$name\"].version // \"\"" "$STATE_FILE" 2>/dev/null
    else
        fail "需要 python3 或 jq 来读取 .build-state.json"
    fi
}

# ── read remote version via SSH ──────────────────────────────────────────
remote_version() {
    local name="$1"
    ssh "$REMOTE_HOST" "cat $REMOTE_PATH/$name-version.txt 2>/dev/null" 2>/dev/null | tr -d '[:space:]' || echo ""
}

# ── main ─────────────────────────────────────────────────────────────────
echo ""
echo "  huayu deploy system"
echo "  ─────────────────────────────────────────────────────"
if [ "$DRY_RUN" = true ]; then
    echo "  (dry run — no files will be copied)"
fi
echo ""

step "Checking remote versions on $REMOTE_HOST ..."

COMPONENTS=("huayu" "codex" "claude")
TO_DEPLOY=()

for name in "${COMPONENTS[@]}"; do
    local_ver=$(local_version "$name")
    remote_ver=$(remote_version "$name")

    if [ -z "$local_ver" ]; then
        skip "$name — not built locally"
        continue
    fi

    if [ "$FORCE" = false ] && [ "$local_ver" = "$remote_ver" ]; then
        skip "$name  local=$local_ver  remote=$remote_ver"
        continue
    fi

    remote_display="${remote_ver:-"(none)"}"
    deploy "$name  local=$local_ver  remote=$remote_display"
    TO_DEPLOY+=("$name")
done

echo ""

if [ ${#TO_DEPLOY[@]} -eq 0 ]; then
    echo "  All versions match remote — nothing to deploy."
    echo ""
    exit 0
fi

if [ "$DRY_RUN" = true ]; then
    echo "  ${#TO_DEPLOY[@]} component(s) would be deployed: ${TO_DEPLOY[*]}"
    echo ""
    exit 0
fi

# ── deploy files ─────────────────────────────────────────────────────────
[ -d "$RELEASE_DIR" ] || fail "release/ directory not found — run build.ps1 first"

# Always deploy installer scripts (small, ensures consistency)
ALWAYS_DEPLOY=()
for f in "$RELEASE_DIR"/huayu.ps1 "$RELEASE_DIR"/huayu.sh; do
    [ -f "$f" ] && ALWAYS_DEPLOY+=("$f")
done

if [ ${#ALWAYS_DEPLOY[@]} -gt 0 ]; then
    step "scp installer scripts ..."
    scp "${ALWAYS_DEPLOY[@]}" "$REMOTE_HOST:$REMOTE_PATH/"
    ok "installer scripts"
fi

scp_file() {
    local f="$1"
    if [ ! -f "$f" ]; then
        echo "  [!] Missing: $f — skipping"
        return
    fi
    local fname
    fname=$(basename "$f")
    step "scp $fname → $REMOTE_HOST:$REMOTE_PATH/"
    scp "$f" "$REMOTE_HOST:$REMOTE_PATH/"
}

for name in "${TO_DEPLOY[@]}"; do
    local_ver=$(local_version "$name")
    deployed=0

    # scp all files matching this component name
    for f in "$RELEASE_DIR"/$name-* "$RELEASE_DIR"/$name.*; do
        [ -f "$f" ] || continue
        scp_file "$f"
        deployed=1
    done

    if [ "$deployed" -eq 1 ]; then
        ok "$name $local_ver deployed"
    else
        echo "  [!] No release files found for $name"
    fi
done

echo ""
echo "  ─────────────────────────────────────────────────────"
echo "  ${#TO_DEPLOY[@]} component(s) deployed: ${TO_DEPLOY[*]}"
echo ""
