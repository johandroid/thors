# Multi-stage build for the single Leptos (SSR) binary.
# Builds with `cargo leptos build --release` and packages the server plus static assets.

### Builder ############################################################
FROM rust:1.91 AS builder

# System deps needed for Diesel (libpq) and tonic gRPC (protoc)
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    pkg-config \
    libssl-dev \
    libpq-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Leptos toolchain
RUN rustup target add wasm32-unknown-unknown \
    && cargo install cargo-leptos --locked

WORKDIR /app

# Caching: copy manifests first
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY style ./style
COPY migrations ./migrations

# Build server + client assets
RUN cargo leptos build --release

### Runtime ############################################################
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libpq5 \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy server binary and static site produced by cargo-leptos
COPY --from=builder /app/target/release/thors /app/thors
COPY --from=builder /app/target/site /app/site

# Leptos runtime configuration
ENV LEPTOS_OUTPUT_NAME=thors \
    LEPTOS_SITE_ROOT=/app/site \
    LEPTOS_SITE_PKG_DIR=pkg \
    LEPTOS_SITE_ADDR=0.0.0.0:3000 \
    RUST_LOG=info

EXPOSE 3000

CMD ["./thors"]
