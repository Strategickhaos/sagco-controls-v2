# ── SAGCO Controls v2 — Multi-stage Docker build ──────────────────────────────
# Stage 1: Builder — compiles all Rust bins
FROM rust:1.96-slim AS builder

WORKDIR /sagco

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Cache dependencies first
COPY Cargo.toml Cargo.lock* ./
RUN mkdir -p src/bin src/models src/modules && \
    echo 'fn main() {}' > src/main.rs

# Build deps only (cache layer)
RUN cargo build --release 2>/dev/null || true
RUN rm -rf src

# Copy full source
COPY src/ src/

# Build all bins
RUN cargo build --release

# ── Stage 2: Runtime — minimal image with all SAGCO bins ──────────────────────
FROM debian:trixie-slim AS runtime

WORKDIR /sagco

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy all compiled bins
COPY --from=builder /sagco/target/release/sagco-reclass     /usr/local/bin/
COPY --from=builder /sagco/target/release/sagco-agent       /usr/local/bin/
COPY --from=builder /sagco/target/release/sagco-guard       /usr/local/bin/
COPY --from=builder /sagco/target/release/sagco-observe     /usr/local/bin/
COPY --from=builder /sagco/target/release/sagco-tokenize    /usr/local/bin/
COPY --from=builder /sagco/target/release/sagco-binscan     /usr/local/bin/
COPY --from=builder /sagco/target/release/sagco-extract     /usr/local/bin/
COPY --from=builder /sagco/target/release/sagco-fswalk      /usr/local/bin/
COPY --from=builder /sagco/target/release/sagco-timeline    /usr/local/bin/
COPY --from=builder /sagco/target/release/sagco-hunt        /usr/local/bin/
COPY --from=builder /sagco/target/release/sagco-chainverify /usr/local/bin/
COPY --from=builder /sagco/target/release/sagco-baseline    /usr/local/bin/
COPY --from=builder /sagco/target/release/sagco-creep-watch /usr/local/bin/
COPY --from=builder /sagco/target/release/sagco-verify      /usr/local/bin/
COPY --from=builder /sagco/target/release/sagco-forecast    /usr/local/bin/
COPY --from=builder /sagco/target/release/sagco-topoopt     /usr/local/bin/
COPY --from=builder /sagco/target/release/sagco-topofuzz    /usr/local/bin/
COPY --from=builder /sagco/target/release/sagco-topology    /usr/local/bin/
COPY --from=builder /sagco/target/release/sagco-stepper     /usr/local/bin/

# Runtime directories
RUN mkdir -p /sagco/reports /sagco/data

# Seed a default hunt rules file
RUN printf '# SAGCO default rules\nSAGCO_STATUS: SAGCO_[A-Z_]+\nSCOPE_CREEP: SCOPE_CREEP\nHIGH_ENTROPY: [0-9a-fA-F]{64}\nURL_FOUND: https?://[^\\s]+\n' \
    > /sagco/data/sagco_default.rules

LABEL org.opencontainers.image.title="sagco-controls-v2" \
      org.opencontainers.image.description="SAGCO Sovereign Forensic + Audit Engine" \
      org.opencontainers.image.authors="Domenic Gabriel Garza / Strategickhaos DAO LLC" \
      sagco.version="0.3.0" \
      sagco.team="red+blue+purple"

CMD ["sagco-reclass", "--help"]
