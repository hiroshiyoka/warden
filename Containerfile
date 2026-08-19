# Syntax: https://docs.docker.com/reference/dockerfile/

FROM rust:1.97-bookworm AS builder
WORKDIR /app
COPY Cargo.toml ./
COPY warden-api warden-api
RUN cargo build --release --workspace

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/warden-api /usr/local/bin/warden-api
EXPOSE 8080
CMD ["warden-api"]
