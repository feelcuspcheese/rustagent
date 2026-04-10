FROM rust:1.85-slim-bookworm AS builder
RUN apt-get update && apt-get install -y \
    cmake ninja-build pkg-config perl libclang-dev musl-tools \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN cargo fetch
COPY . .
RUN rustup target add x86_64-unknown-linux-musl
RUN cargo build --release --target x86_64-unknown-linux-musl

FROM alpine:latest
RUN apk add --no-cache ca-certificates
WORKDIR /root/
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/appointment-agent /usr/local/bin/agent
COPY configs/default_config.yaml /root/configs/default_config.yaml
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh
EXPOSE 8080
ENTRYPOINT ["/entrypoint.sh"]
