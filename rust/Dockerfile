# syntax=docker/dockerfile:1.4

FROM rust:bookworm AS builder

WORKDIR /usr/src/app

# Copy manifests first
COPY Cargo.toml Cargo.lock ./
COPY ws-server/Cargo.toml ws-server/Cargo.toml
COPY shared/Cargo.toml shared/Cargo.toml

# Dummy crates for workspace members, which aren't needed
RUN for crate in client wasm-client; do \
      mkdir -p $crate/src && \
      echo '[package]' > $crate/Cargo.toml && \
      echo "name = \"${crate}\"" >> $crate/Cargo.toml && \
      echo 'version = "0.0.0"' >> $crate/Cargo.toml && \
      echo 'edition = "2021"' >> $crate/Cargo.toml && \
      echo 'fn main() {}' > $crate/src/main.rs; \
    done

# Pre-build deps (cached)
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/usr/src/app/target \
    cargo build --release --package ws-server || true

# Copy actual source code AFTER caching deps
COPY shared ./shared
COPY ws-server ./ws-server

# Final build using cached deps
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/usr/src/app/target \
    cargo build --release --package ws-server && \
    cp target/release/ws-server /usr/src/app/ws-server-binary

FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y openssl ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/app/ws-server-binary /usr/local/bin/ws-server

EXPOSE 8080

CMD ["ws-server"]
