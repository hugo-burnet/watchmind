FROM rust:1.97-bookworm AS builder
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY apps/watchmind-api apps/watchmind-api
COPY apps/watchmind-cli apps/watchmind-cli
COPY crates crates
RUN cargo build --release -p watchmind-api

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /src/target/release/watchmind-api /usr/local/bin/watchmind-api
ENV WATCHMIND_DATA_DIR=/data WATCHMIND_BIND=0.0.0.0:3000 WATCHMIND_TRUST_PROXY=1
VOLUME ["/data"]
EXPOSE 3000
CMD ["watchmind-api"]
