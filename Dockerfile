FROM rust:1-bookworm AS builder
WORKDIR /build
COPY . .
RUN cargo build --locked --release --bin trading-engine

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 trading
COPY --from=builder /build/target/release/trading-engine /usr/local/bin/trading-engine
USER trading
ENV RUST_LOG=info
ENTRYPOINT ["/usr/local/bin/trading-engine"]
