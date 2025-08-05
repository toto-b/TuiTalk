FROM rust:latest AS builder

WORKDIR /usr/src/app
COPY . .

RUN cargo build --release --bin ws-server

FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y openssl ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/app/target/release/ws-server /usr/local/bin/

EXPOSE 8080

CMD ["ws-server"]
