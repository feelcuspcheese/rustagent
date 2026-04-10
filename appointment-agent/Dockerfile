# Version: 0.1.1
# Stage 1: Cargo Chef - Prepare dependency recipe
FROM rust:1.85-slim-bookworm AS chef
RUN cargo install cargo-chef
WORKDIR /app

# Stage 2: Planner
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Builder
FROM chef AS builder
# Install BoringSSL build dependencies (cmake, perl, clang)
RUN apt-get update && apt-get install -y \
    cmake \
    ninja-build \
    pkg-config \
    perl \
    libclang-dev \
    musl-tools \
    g++ \
    && rm -rf /var/lib/apt/lists/*

COPY --from=planner /app/recipe.json recipe.json
RUN rustup target add x86_64-unknown-linux-musl

# Build dependencies only (cached layer)
RUN cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json

# Build application
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl

# Stage 4: Final Runtime Image
FROM alpine:3.19
RUN apk add --no-cache ca-certificates tzdata

WORKDIR /root/
# Create config directory
RUN mkdir -p /root/configs

# Copy binary from builder
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/appointment-agent /usr/local/bin/agent
# Copy default config and entrypoint
COPY configs/default_config.yaml /root/configs/default_config.yaml
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

EXPOSE 8080

# Use the entrypoint script to handle configuration initialization
ENTRYPOINT ["/entrypoint.sh"]
