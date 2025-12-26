FROM rust:1.92-slim-bullseye AS builder

WORKDIR /app
RUN USER=root cargo new --bin dock
WORKDIR /app/dock

COPY Cargo.toml Cargo.lock* ./
RUN cargo build --release || true

COPY src ./src
COPY config.json ./

RUN cargo build --release

FROM debian:bullseye-slim AS runtime

WORKDIR /app

COPY --from=builder /app/dock/target/release/dock /usr/local/bin/dock
COPY config.json ./config.json
EXPOSE 21
ENTRYPOINT ["dock"]
