#!/usr/bin/env bash
# Static guard for the agent rules that can be checked without a database.
# Run in CI and before a commit. Exit 1 if any rule is broken.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

FAILED=0
fail() {
    echo "agent-rule guard: $1" >&2
    FAILED=1
}

# Handlers are HTTP-only: no SQLx and no panic helpers.
if grep -RIn --include='handlers.rs' 'sqlx::' src/modules >/dev/null 2>&1; then
    fail "SQLx found in a handlers.rs file (keep SQL in service.rs)"
fi
if grep -RInE --include='handlers.rs' '\.(unwrap|expect)\(' src/modules >/dev/null 2>&1; then
    fail "unwrap/expect found in a handlers.rs file (propagate AppError)"
fi

# DTOs must never expose storage keys.
if grep -RInE --include='dtos.rs' 'storage_path|final_asset_path' src/modules >/dev/null 2>&1; then
    fail "storage key field found in a DTO"
fi

# No debug printing in production source.
if grep -RInE 'println!|dbg!' src --include='*.rs' >/dev/null 2>&1; then
    fail "println!/dbg! found in src (use tracing or remove it)"
fi

# Secrets must never be tracked.
if git ls-files --error-unmatch .env >/dev/null 2>&1; then
    fail ".env is tracked by git"
fi

# Append-only migration sequence: 0001, 0002, ... with no gaps.
expected=1
for file in migrations/*.sql; do
    base="$(basename "$file")"
    number="${base%%_*}"
    if ! [[ "$number" =~ ^[0-9]{4}$ ]]; then
        fail "migration '$base' does not start with a four-digit number"
        continue
    fi
    printf -v wanted '%04d' "$expected"
    if [ "$number" != "$wanted" ]; then
        fail "migration sequence expects $wanted, found $base"
    fi
    expected=$((expected + 1))
done

# The always-loaded rule file stays short.
agents_lines="$(wc -l < AGENTS.md)"
if [ "$agents_lines" -gt 160 ]; then
    fail "AGENTS.md is $agents_lines lines; move details into docs/rules/"
fi

# The guide's required files exist.
for file in \
    CODEBASE_MAP.md \
    docs/agent-master-guide.md \
    docs/decisions.md \
    CHANGELOG.md \
    UPDATE_NOTES.md; do
    if [ ! -f "$file" ]; then
        fail "missing required file: $file"
    fi
done

if [ "$FAILED" -ne 0 ]; then
    exit 1
fi

echo "agent-rule guards passed"
