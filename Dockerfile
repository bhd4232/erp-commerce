# syntax=docker/dockerfile:1

# ---- build -----------------------------------------------------------------
FROM rust:1.82-slim-bookworm AS builder

# aws-lc-sys (pulled in by rustls) needs a C toolchain and cmake.
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential cmake clang pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Cache dependencies before copying sources.
COPY Cargo.toml Cargo.lock* ./
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs \
    && cargo build --release \
    && rm -rf src

COPY src ./src
COPY migrations ./migrations

# sqlx::migrate! embeds migrations/ at compile time, so it must be present.
RUN touch src/main.rs && cargo build --release

# ---- runtime ---------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 app

COPY --from=builder /app/target/release/erp-commerce /usr/local/bin/erp-commerce

USER app
EXPOSE 8080

ENV APP_HOST=0.0.0.0 \
    APP_PORT=8080 \
    RUST_LOG=info

HEALTHCHECK --interval=15s --timeout=3s --start-period=10s --retries=3 \
    CMD curl -fsS http://127.0.0.1:8080/healthz || exit 1

ENTRYPOINT ["/usr/local/bin/erp-commerce"]
