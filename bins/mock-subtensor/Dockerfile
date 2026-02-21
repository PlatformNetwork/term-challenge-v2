# =============================================================================
# Mock Subtensor Docker Image
# =============================================================================
# Standalone Bittensor RPC mock for testing validator nodes
# Simulates a full Subtensor node with:
# - WebSocket JSON-RPC 2.0 server (substrate-compatible)
# - Simulated block production (12s tempo)
# - 256 synthetic validators with realistic stake distribution
# - Commit-reveal weight mechanism
# =============================================================================

FROM rust:1.82-bookworm AS builder

# Install dependencies
RUN apt-get update \
    && apt-get install -y \
        pkg-config \
        libssl-dev \
        protobuf-compiler \
        cmake \
        clang \
        libclang-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY bins ./bins

# Build release binary
RUN cargo build --release --bin mock-subtensor

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/mock-subtensor /usr/local/bin/mock-subtensor

# Create data directory
RUN mkdir -p /data && chmod 755 /data

# Environment defaults
ENV RUST_LOG=info,mock_subtensor=debug
ENV MOCK_SUBTENSOR_BIND=0.0.0.0:9944
ENV MOCK_SUBTENSOR_TEMPO=12
ENV MOCK_SUBTENSOR_NETUID=100
ENV MOCK_SUBTENSOR_VALIDATOR_COUNT=256
ENV MOCK_SUBTENSOR_COMMIT_REVEAL=true

# Expose ports
EXPOSE 9944
EXPOSE 9933

# Health check
HEALTHCHECK --interval=10s --timeout=5s --start-period=15s --retries=3 \
    CMD curl -f http://localhost:9944/health || exit 1

# Default entrypoint
ENTRYPOINT ["/usr/local/bin/mock-subtensor"]

# Default arguments (can be overridden)
CMD ["--bind", "0.0.0.0:9944", "--tempo", "12", "--netuid", "100", "--validator-count", "256"]
