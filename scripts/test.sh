#!/usr/bin/env bash
# Local test runner.
#
# Each `#[sqlx::test]` creates its own database and runs every migration, so
# parallel test binaries are the main memory spike on a small machine. This
# script starts a low-memory PostgreSQL when one is not already running, runs
# the suite serially, and stops the server it started.
#
#   ./scripts/test.sh                 # all tests, 1 thread
#   TEST_THREADS=2 ./scripts/test.sh  # allow more parallelism
#   KEEP_PG=1 ./scripts/test.sh       # leave PostgreSQL running afterwards
#   ./scripts/test.sh digital_flow    # pass a filter through to cargo
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PGDATA="${PGDATA:-/tmp/opencode/pgdata}"
PGPORT="${PGPORT:-5432}"
PGBIN="${PGBIN:-/usr/lib/postgresql/16/bin}"
TEST_THREADS="${TEST_THREADS:-1}"
DATABASE_URL="${DATABASE_URL:-postgres://postgres:postgres@127.0.0.1:${PGPORT}/erp_commerce}"
export DATABASE_URL

find_bin() {
    if [ -n "$PGBIN" ] && [ -x "$PGBIN/$1" ]; then
        echo "$PGBIN/$1"
    elif command -v "$1" >/dev/null 2>&1; then
        command -v "$1"
    else
        echo ""
    fi
}

READY="$(find_bin pg_isready)"
PSQL="$(find_bin psql)"
CREATEDB="$(find_bin createdb)"
STARTED=0

is_up() {
    [ -n "$READY" ] && "$READY" -h 127.0.0.1 -p "$PGPORT" >/dev/null 2>&1
}

start_pg() {
    if [ "$(id -u)" != "0" ]; then
        echo "PostgreSQL is not running. Start it first (e.g. sudo systemctl start postgresql)," >&2
        echo "or run this script as root with PGDATA/PGBIN pointing at a cluster." >&2
        exit 2
    fi
    if [ -z "$PGBIN" ] || [ ! -x "$PGBIN/pg_ctl" ]; then
        echo "PostgreSQL is not running and pg_ctl was not found (set PGBIN)." >&2
        exit 2
    fi
    if [ ! -d "$PGDATA" ]; then
        echo "PostgreSQL is not running and there is no cluster at $PGDATA." >&2
        echo "Start your system PostgreSQL, or point PGDATA at a cluster." >&2
        exit 2
    fi

    echo "starting PostgreSQL (low-memory profile) ..."
    su postgres -c "$PGBIN/pg_ctl -D $PGDATA -l /tmp/opencode/pg.log \
        -o '-p $PGPORT -k /tmp/opencode -c listen_addresses=127.0.0.1 \
        -c shared_buffers=32MB -c max_connections=20 \
        -c work_mem=2MB -c maintenance_work_mem=24MB' start" >/dev/null

    for _ in $(seq 1 40); do
        is_up && return 0
        sleep 0.5
    done
    echo "PostgreSQL did not become ready." >&2
    exit 1
}

cleanup() {
    if [ "$STARTED" = "1" ] && [ "${KEEP_PG:-0}" != "1" ]; then
        echo "stopping PostgreSQL ..."
        su postgres -c "$PGBIN/pg_ctl -D $PGDATA stop -m fast" >/dev/null 2>&1 || true
    fi
}
trap cleanup EXIT

if ! is_up; then
    start_pg
    STARTED=1
fi

if [ -n "$PSQL" ] && ! "$PSQL" "$DATABASE_URL" -tAc 'select 1' >/dev/null 2>&1; then
    if [ -n "$CREATEDB" ]; then
        "$CREATEDB" -h 127.0.0.1 -p "$PGPORT" -U postgres erp_commerce
    fi
fi

echo "running tests (threads=$TEST_THREADS)"
cargo test --all-features -- --test-threads="$TEST_THREADS" "$@"
