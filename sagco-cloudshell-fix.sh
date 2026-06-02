#!/bin/bash
# ═══════════════════════════════════════════════════════════════════════════════
# SAGCO Cloud Shell Fix — paste this directly into Cloud Shell
# Fixes: disk space + Rust toolchain + source upload
# No billing required — runs entirely on free Cloud Shell compute
# ═══════════════════════════════════════════════════════════════════════════════

echo "=== SAGCO CLOUD SHELL RECOVERY ==="
echo "PROJECT=$(gcloud config get-value project)"
echo ""

# ── FIX 1: Clear disk space ───────────────────────────────────────────────────
echo "--- DISK CLEANUP ---"
df -h ~ | tail -1

# Remove largest space consumers in Cloud Shell
rm -rf ~/.rustup/toolchains/*/share/doc   2>/dev/null || true
rm -rf ~/.rustup/toolchains/*/share/man   2>/dev/null || true
rm -rf ~/.rustup/tmp/*                    2>/dev/null || true
rm -rf ~/go/pkg/mod/cache                 2>/dev/null || true
rm -rf ~/.cache/pip                       2>/dev/null || true
rm -rf ~/.npm/_npx                        2>/dev/null || true
rm -rf ~/sagco-home/target               2>/dev/null || true

# Show what's using space
echo "TOP_DIRS:"
du -sh ~/* 2>/dev/null | sort -rh | head -10

df -h ~ | tail -1
echo ""

# ── FIX 2: Rust toolchain — use existing rustup, skip reinstall ───────────────
echo "--- RUST TOOLCHAIN ---"
export PATH="$HOME/.cargo/bin:$PATH"
source "$HOME/.cargo/env" 2>/dev/null || true

if command -v rustc &>/dev/null; then
    echo "RUST_VERSION=$(rustc --version)"
    # Update in-place without re-downloading docs/man
    rustup set profile minimal
    rustup update stable 2>/dev/null || echo "RUSTUP_UPDATE_SKIPPED=disk_constraint"
    echo "CARGO_VERSION=$(cargo --version)"
else
    echo "RUST_MISSING — need to clear more disk first"
    echo "Run: du -sh ~/* | sort -rh  to find large dirs"
    exit 1
fi
echo ""

# ── FIX 3: Source — check what's in sagco-home ───────────────────────────────
echo "--- SOURCE STATE ---"
ls -la ~/sagco-home/ 2>/dev/null || echo "sagco-home_EMPTY"

if [ ! -f ~/sagco-home/Cargo.toml ]; then
    echo ""
    echo "SOURCE_MISSING — upload from Windows with:"
    echo ""
    echo "  # From Windows PowerShell:"
    echo '  gcloud cloud-shell scp --recurse localhost:"C:/Users/garza/Downloads/Mobile Devices/src" sagco-oscomputconsciousness:~/sagco-home/'
    echo '  gcloud cloud-shell scp localhost:"C:/Users/garza/Downloads/Mobile Devices/Cargo.toml" sagco-oscomputconsciousness:~/sagco-home/'
    echo '  gcloud cloud-shell scp localhost:"C:/Users/garza/Downloads/Mobile Devices/Cargo.lock" sagco-oscomputconsciousness:~/sagco-home/'
    echo ""
    echo "  # Or use the Cloud Shell editor Upload button"
    exit 0
fi

# ── BUILD (only if source present and disk has room) ─────────────────────────
echo ""
echo "--- BUILD ---"
FREE_KB=$(df ~ | tail -1 | awk '{print $4}')
echo "FREE_KB=${FREE_KB}"

if [ "$FREE_KB" -lt 1500000 ]; then
    echo "DISK_TOO_FULL — need ~1.5GB free for Rust build"
    echo "RUN: rm -rf ~/sagco-home/target ~/.rustup/toolchains/*/share"
    exit 1
fi

cd ~/sagco-home
cargo build --release 2>&1 | tail -5
echo ""

# ── VERIFY binaries ───────────────────────────────────────────────────────────
echo "--- BINARIES ---"
ls -lh ~/sagco-home/target/release/sagco-* 2>/dev/null | awk '{print $NF, $5}'

# ── ADD TO PATH ───────────────────────────────────────────────────────────────
export PATH="$HOME/sagco-home/target/release:$PATH"

# ── FIRST AGENT RUN ───────────────────────────────────────────────────────────
echo ""
echo "--- SAGCO AGENT BOOT ---"
cd ~/sagco-home
./target/release/sagco-guard 4 10
echo ""
./target/release/sagco-observe Cargo.toml
echo ""
./target/release/sagco-reclass 1011.6 68.4 851.6 228.4 4 10 | grep "STATUS=\|SEAL="
echo ""
echo "STATUS=SAGCO_CLOUDSHELL_NODE_ONLINE"
echo "NODE=$(hostname)"
echo "HOME=~/sagco-home"
