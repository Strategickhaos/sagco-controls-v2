#!/bin/bash
# ═══════════════════════════════════════════════════════════════════════════════
# SAGCO Cloud Shell Bootstrap — sagco-oscomputconsciousness
# Paste this into Cloud Shell to build the agent home from scratch.
# garza.domenic101@gmail.com / Strategickhaos DAO LLC
# ═══════════════════════════════════════════════════════════════════════════════
set -euo pipefail

PROJECT="sagco-oscomputconsciousness"
REGION="us-central1"
REPO="sagco-agents"
IMAGE_LOCAL="sagco-controls-v2:latest"
IMAGE_REMOTE="us-central1-docker.pkg.dev/${PROJECT}/${REPO}/sagco-controls-v2:0.3.0"
HOME_DIR="$HOME/sagco-home"

echo "=== SAGCO CLOUD SHELL INIT ==="
echo "PROJECT=${PROJECT}"
echo "HOME_DIR=${HOME_DIR}"
echo ""

# ── Step 1: Confirm identity ─────────────────────────────────────────────────
echo "--- IDENTITY ---"
gcloud config set project "${PROJECT}"
gcloud auth list

# ── Step 2: Enable required APIs ─────────────────────────────────────────────
echo ""
echo "--- ENABLING APIs ---"
gcloud services enable \
  artifactregistry.googleapis.com \
  run.googleapis.com \
  container.googleapis.com \
  cloudresourcemanager.googleapis.com \
  --project="${PROJECT}" \
  --quiet
echo "APIs_ENABLED=OK"

# ── Step 3: Create Artifact Registry repo ────────────────────────────────────
echo ""
echo "--- ARTIFACT REGISTRY ---"
gcloud artifacts repositories create "${REPO}" \
  --repository-format=docker \
  --location="${REGION}" \
  --description="SAGCO sovereign agent images" \
  --project="${PROJECT}" \
  --quiet 2>/dev/null || echo "REPO_EXISTS=${REPO}"

gcloud auth configure-docker "us-central1-docker.pkg.dev" --quiet

# ── Step 4: Install Rust in Cloud Shell ──────────────────────────────────────
echo ""
echo "--- RUST INSTALL ---"
if ! command -v cargo &>/dev/null; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --quiet
  source "$HOME/.cargo/env"
  echo "RUST_INSTALLED=OK"
else
  echo "RUST_ALREADY_INSTALLED=$(rustc --version)"
fi

# ── Step 5: Clone / sync SAGCO source ────────────────────────────────────────
echo ""
echo "--- SAGCO HOME SETUP ---"
mkdir -p "${HOME_DIR}"/{data,reports,reports/agent,reports/gcp,reports/k8s,reports/observe,reports/topology}

# If source not present, pull from Container Registry or build inline
if [ ! -f "${HOME_DIR}/Cargo.toml" ]; then
  echo "SOURCE_NOT_FOUND — copy your project to ${HOME_DIR}"
  echo "  From local: gcloud cloud-shell scp --recurse localhost:\"C:/Users/garza/Downloads/Mobile Devices\" ${PROJECT}:~/sagco-home"
fi

# ── Step 6: Set up persistent .bashrc additions ───────────────────────────────
echo ""
echo "--- CONFIGURING SHELL ---"
BASHRC_BLOCK='
# ── SAGCO Sovereign Environment ──────────────────────────────────────────────
export SAGCO_HOME="$HOME/sagco-home"
export PATH="$SAGCO_HOME/target/release:$PATH"

alias sagco-reclass="$SAGCO_HOME/target/release/sagco-reclass"
alias sagco-agent="$SAGCO_HOME/target/release/sagco-agent"
alias sagco-observe="$SAGCO_HOME/target/release/sagco-observe"
alias sagco-guard="$SAGCO_HOME/target/release/sagco-guard"
alias sagco-topology="$SAGCO_HOME/target/release/sagco-topology"
alias sagco-forecast="$SAGCO_HOME/target/release/sagco-forecast"
alias sagco-gcp-agent="$SAGCO_HOME/target/release/sagco-gcp-agent"
alias sagco-k8s-observe="$SAGCO_HOME/target/release/sagco-k8s-observe"

sagco_status() {
  echo "=== SAGCO NODE STATUS ==="
  echo "PROJECT=$(gcloud config get-value project 2>/dev/null)"
  echo "CONTEXT=$(kubectl config current-context 2>/dev/null || echo NONE)"
  echo "NODE=$(hostname)"
  echo "SAGCO_HOME=$SAGCO_HOME"
  ls "$SAGCO_HOME/data/"*.jsonl 2>/dev/null | while read f; do
    echo "LEDGER=$f ENTRIES=$(wc -l < "$f")"
  done
}

echo "SAGCO Node Online — sagco-oscomputconsciousness"
# ─────────────────────────────────────────────────────────────────────────────
'

if ! grep -q "SAGCO Sovereign Environment" "$HOME/.bashrc" 2>/dev/null; then
  echo "$BASHRC_BLOCK" >> "$HOME/.bashrc"
  echo "BASHRC_UPDATED=OK"
else
  echo "BASHRC_ALREADY_SET"
fi

# ── Step 7: Create default hunt rules ────────────────────────────────────────
cat > "${HOME_DIR}/data/sagco_default.rules" <<'RULES'
# SAGCO default hunt rules — Cloud Shell
SAGCO_STATUS: SAGCO_[A-Z_]+
SCOPE_CREEP: SCOPE_CREEP
HIGH_ENTROPY: [0-9a-fA-F]{64}
URL_FOUND: https?://[^\s]+
GCP_PROJECT: sagco-oscomputconsciousness
SERVICE_ACCOUNT: @.*\.iam\.gserviceaccount\.com
RULES
echo "RULES_FILE=${HOME_DIR}/data/sagco_default.rules"

# ── Step 8: GKE context import ───────────────────────────────────────────────
echo ""
echo "--- K8S CONTEXT ---"
gcloud container clusters get-credentials jarvis-swarm-personal-001 \
  --region "${REGION}" \
  --project jarvis-swarm-personal 2>/dev/null && echo "K8S_CONTEXT=jarvis-swarm-personal-001" \
  || echo "K8S_CONTEXT_SKIP=jarvis-cluster-not-reachable-from-this-project"

# ── Step 9: Push Docker image to Artifact Registry (if available) ────────────
echo ""
echo "--- ARTIFACT PUSH ---"
if docker image inspect "${IMAGE_LOCAL}" &>/dev/null 2>&1; then
  docker tag "${IMAGE_LOCAL}" "${IMAGE_REMOTE}"
  docker push "${IMAGE_REMOTE}"
  echo "IMAGE_PUSHED=${IMAGE_REMOTE}"
else
  echo "IMAGE_NOT_LOCAL=pull_or_build_first"
  echo "BUILD_CMD=cd ${HOME_DIR} && cargo build --release"
  echo "PUSH_CMD=docker build -t ${IMAGE_REMOTE} . && docker push ${IMAGE_REMOTE}"
fi

# ── Status ────────────────────────────────────────────────────────────────────
echo ""
echo "========================================================"
echo "STATUS=SAGCO_CLOUDSHELL_HOME_READY"
echo "NEXT_STEPS:"
echo "  1. source ~/.bashrc"
echo "  2. cd ~/sagco-home && cargo build --release"
echo "  3. sagco_status"
echo "  4. sagco-agent run all --target Cargo.toml"
echo "  5. sagco-topology --json --dot"
echo "========================================================"
