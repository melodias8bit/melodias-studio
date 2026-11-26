# Rust stack build. Builder compiles the release binary; the runtime image
# carries only ca-certificates plus the binary. Multi-arch comes from the
# official rust and debian manifests; build with
# `docker buildx build --platform linux/amd64,linux/arm64` when needed.
FROM rust:1.100.0-slim-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
RUN cargo build --locked --release

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/melodias /usr/local/bin/app

ENTRYPOINT ["/usr/local/bin/app"]
