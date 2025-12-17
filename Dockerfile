# ============================================================================
# Term Challenge - Optimized Multi-stage Docker Build
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
RUN strip /app/target/release/term /app/target/release/term-server

# Stage 2: Runtime - Minimal production image
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
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binaries from builder
COPY --from=builder /app/target/release/term /usr/local/bin/
COPY --from=builder /app/target/release/term-server /usr/local/bin/

# Copy SDK for agent development
COPY sdk /app/sdk

# Create directories
RUN mkdir -p /data /app/benchmark_results

# Environment
ENV RUST_LOG=info,term_challenge=debug
ENV DATA_DIR=/data

# Expose ports (if needed for RPC)
EXPOSE 8080

# Use tini as init system
ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["term-server"]

# Labels
LABEL org.opencontainers.image.source="https://github.com/PlatformNetwork/term-challenge"
LABEL org.opencontainers.image.description="Term Challenge - Terminal Benchmark for AI Agents"
LABEL org.opencontainers.image.licenses="MIT"
