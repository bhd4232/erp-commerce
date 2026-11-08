# Developer task runner. `just` reads this file; every recipe is also a
# plain command documented in AGENTS.md.

set dotenv-load := true
set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

default:
    @just --list

# --- infrastructure ---------------------------------------------------------

up:
    docker compose up -d
    @echo "postgres :5432  redis :6379  minio :9000 (console :9001)"

down:
    docker compose down

reset:
    docker compose down -v
    docker compose up -d

logs:
    docker compose logs -f --tail=100

# --- backend ----------------------------------------------------------------

dev:
    RUST_LOG=info,tower_http=debug cargo run

build:
    cargo build

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

lint:
    cargo clippy --all-targets --all-features -- -D warnings

test:
    cargo test --all-features

# Applies migrations against DATABASE_URL.
migrate:
    sqlx migrate run

migrate-add name:
    sqlx migrate add {{name}}

# Recreates the database from scratch. Destructive.
migrate-redo:
    sqlx database drop -y
    sqlx database create
    sqlx migrate run

check: fmt-check lint test

# --- frontend ---------------------------------------------------------------

fe-install:
    cd frontend && npm ci

fe-dev:
    cd frontend && npm run dev

fe-build:
    cd frontend && npm run build

fe-lint:
    cd frontend && npm run lint

# --- desktop ----------------------------------------------------------------

tauri-dev:
    cd frontend && npm run tauri:dev

tauri-build:
    cd frontend && npm run tauri build

# --- housekeeping -----------------------------------------------------------

clean:
    cargo clean
    rm -rf frontend/dist frontend/node_modules
