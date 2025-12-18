# ============================================================================
# Term Challenge - Optimized Multi-stage Docker Build
# ============================================================================
# This image is used by platform validators to run the term-challenge server
# Image: ghcr.io/platformnetwork/term-challenge:latest
# ============================================================================

# Stage 1: Builder
FROM rust:slim-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy source code
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY bin ./bin
COPY tests ./tests

# Build release binaries (CLI and Server)
RUN cargo build --release --bin term --bin term-server

# Strip binaries for smaller size
RUN strip /app/target/release/term /app/target/release/term-server 2>/dev/null || true

# Stage 2: Runtime - Production image for platform validators
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    curl \
    docker.io \
    python3 \
    python3-pip \
    nodejs \
    npm \
    git \
    tmux \
    tini \
    && rm -rf /var/lib/apt/lists/* \
    && rm -rf /var/cache/apt/*

WORKDIR /app

# Copy binaries from builder
COPY --from=builder /app/target/release/term /usr/local/bin/
COPY --from=builder /app/target/release/term-server /usr/local/bin/

# Copy SDK for agent development
COPY sdk /app/sdk

# Copy default data and tasks
COPY data /app/data

# Create directories
RUN mkdir -p /data /app/benchmark_results /app/logs

# Non-root user for security (optional, platform may override)
# RUN useradd -m -s /bin/bash termuser && chown -R termuser:termuser /app /data
# USER termuser

# Environment
ENV RUST_LOG=info,term_challenge=debug
ENV DATA_DIR=/data
ENV TERM_CHALLENGE_HOST=0.0.0.0
ENV TERM_CHALLENGE_PORT=8080

# Health check for platform orchestration
# Start period of 30s allows time for Rust binary to initialize
HEALTHCHECK --interval=30s --timeout=10s --start-period=30s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Expose RPC port
EXPOSE 8080

# Use tini as init system for proper signal handling
ENTRYPOINT ["/usr/bin/tini", "--"]

# Default command - run the server
# Platform can override with: docker run ... term-server --host 0.0.0.0 --port 8080
CMD ["term-server", "--host", "0.0.0.0", "--port", "8080"]

# Labels for container registry
LABEL org.opencontainers.image.source="https://github.com/PlatformNetwork/term-challenge"
LABEL org.opencontainers.image.description="Term Challenge - Terminal Benchmark Server for Platform Validators"
LABEL org.opencontainers.image.licenses="MIT"
LABEL org.opencontainers.image.vendor="PlatformNetwork"
