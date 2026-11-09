#!/usr/bin/env bash
# End-to-end smoke test against a running API.
#
#   ./scripts/smoke.sh                      # http://127.0.0.1:8080
#   ./scripts/smoke.sh https://api.example  # any host
#
# Money tests need a way to fund the wallet. Before P3 there is no top-up API,
# so this script calls seed_wallet() through psql. Set DATABASE_URL, or point
# SMOKE_EMAIL / SMOKE_PASSWORD at an already funded account.
set -uo pipefail

API="${1:-${API_BASE:-http://127.0.0.1:8080}}"
API="${API%/}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

PASS=0
FAIL=0
SKIP=0

green() { printf '  \033[32mPASS\033[0m %s\n' "$1"; PASS=$((PASS + 1)); }
red()   { printf '  \033[31mFAIL\033[0m %s — %s\n' "$1" "$2"; FAIL=$((FAIL + 1)); }
skip()  { printf '  \033[33mSKIP\033[0m %s — %s\n' "$1" "$2"; SKIP=$((SKIP + 1)); }

jget() {
    python3 -c 'import sys, json
d = json.load(open(sys.argv[1]))
for k in sys.argv[2].split("."):
    d = d[int(k)] if k.isdigit() else d[k]
print(d)' "$1" "$2"
}

# request METHOD PATH TOKEN BODY OUTFILE -> prints HTTP status
request() {
    local method="$1" path="$2" token="$3" body="$4" out="$5"
    local args=(-sS -o "$out" -w '%{http_code}' -X "$method" "$API$path")
    [ -n "$token" ] && args+=(-H "authorization: Bearer $token")
    [ -n "$body" ] && args+=(-H 'content-type: application/json' -d "$body")
    curl "${args[@]}"
}

echo "API: $API"
echo

# --- liveness ---------------------------------------------------------------
code=$(curl -sS -o "$TMP/health" -w '%{http_code}' --max-time 5 "$API/healthz" 2>/dev/null)
if [ "$code" = "200" ] && [ "$(cat "$TMP/health")" = "ok" ]; then
    green "GET /healthz"
else
    red "GET /healthz" "status ${code:-none}; is the API running?"
    echo
    echo "Start it with: just dev   (or: cargo run)"
    exit 1
fi

# --- identity ---------------------------------------------------------------
EMAIL="${SMOKE_EMAIL:-smoke$(date +%s)@example.com}"
PASSWORD="${SMOKE_PASSWORD:-smoke-password-123}"

if [ -n "${SMOKE_EMAIL:-}" ]; then
    code=$(request POST /api/v1/auth/login "" \
        "{\"email\":\"$EMAIL\",\"password\":\"$PASSWORD\"}" "$TMP/login")
    if [ "$code" = "200" ]; then green "POST /auth/login (existing account)"; else red "POST /auth/login" "HTTP $code"; fi
else
    code=$(request POST /api/v1/auth/register "" \
        "{\"email\":\"$EMAIL\",\"password\":\"$PASSWORD\",\"full_name\":\"Smoke Test\"}" "$TMP/reg")
    if [ "$code" = "201" ]; then green "POST /auth/register"; else red "POST /auth/register" "HTTP $code"; fi
    code=$(request POST /api/v1/auth/login "" \
        "{\"email\":\"$EMAIL\",\"password\":\"$PASSWORD\"}" "$TMP/login")
    if [ "$code" = "200" ]; then green "POST /auth/login"; else red "POST /auth/login" "HTTP $code"; fi
fi

TOKEN=$(jget "$TMP/login" "data.token" 2>/dev/null || true)
USER_ID=$(jget "$TMP/login" "data.user.id" 2>/dev/null || true)
[ -n "$TOKEN" ] || { echo "no token; cannot continue"; exit 1; }

# --- catalogue --------------------------------------------------------------
code=$(request GET "/api/v1/digital/products?per_page=20" "" "" "$TMP/products")
COUNT=$(python3 -c 'import json,sys; print(len(json.load(open(sys.argv[1]))["data"]))' "$TMP/products" 2>/dev/null || echo 0)
if [ "$code" = "200" ] && [ "$COUNT" -ge 1 ]; then
    green "GET /digital/products ($COUNT products)"
else
    red "GET /digital/products" "HTTP $code, count $COUNT; run scripts/seed.sql"
fi
PRODUCT_ID=$(jget "$TMP/products" "data.0.id" 2>/dev/null || true)
PRODUCT_TITLE=$(jget "$TMP/products" "data.0.title" 2>/dev/null || true)

# --- insufficient funds (fresh account) -------------------------------------
if [ -z "${SMOKE_EMAIL:-}" ]; then
    code=$(request POST /api/v1/digital/checkout "$TOKEN" \
        "{\"items\":[{\"product_id\":\"$PRODUCT_ID\",\"quantity\":1}],\"payment_provider\":\"wallet\"}" "$TMP/broke")
    if [ "$code" = "402" ]; then green "checkout with zero balance -> 402"; else red "checkout with zero balance" "HTTP $code"; fi
fi

# --- funding ----------------------------------------------------------------
FUNDED=0
if [ -n "${DATABASE_URL:-}" ] && command -v psql >/dev/null 2>&1 && [ -n "$USER_ID" ]; then
    BALANCE_BEFORE=$(psql "$DATABASE_URL" -tAc "select seed_wallet('$USER_ID', 100.00)" 2>/dev/null || true)
    if [ -n "$BALANCE_BEFORE" ]; then
        green "seed_wallet($USER_ID, 100.00) -> $BALANCE_BEFORE"
        FUNDED=1
    else
        skip "seed_wallet" "psql call failed"
    fi
elif [ -n "${SMOKE_EMAIL:-}" ]; then
    FUNDED=1
    skip "seed_wallet" "using the pre-funded SMOKE_EMAIL account"
else
    skip "seed_wallet" "set DATABASE_URL (with psql) or SMOKE_EMAIL/SMOKE_PASSWORD"
fi

if [ "$FUNDED" = "1" ]; then
    KEY="smoke-$(date +%s)"
    CART="{\"items\":[{\"product_id\":\"$PRODUCT_ID\",\"quantity\":1}],\"payment_provider\":\"wallet\",\"idempotency_key\":\"$KEY\"}"

    code=$(request POST /api/v1/digital/checkout "$TOKEN" "$CART" "$TMP/order")
    if [ "$code" = "201" ]; then green "POST /digital/checkout -> 201"; else red "POST /digital/checkout" "HTTP $code"; fi
    ORDER_ID=$(jget "$TMP/order" "data.order_id" 2>/dev/null || true)
    ORDER_NUMBER=$(jget "$TMP/order" "data.order_number" 2>/dev/null || true)
    BALANCE_AFTER=$(request GET /api/v1/auth/me "$TOKEN" "" "$TMP/me" >/dev/null; jget "$TMP/me" "data.wallet_balance" 2>/dev/null || echo "?")

    code=$(request POST /api/v1/digital/checkout "$TOKEN" "$CART" "$TMP/replay")
    REPLAY_ORDER=$(jget "$TMP/replay" "data.order_id" 2>/dev/null || true)
    BALANCE_REPLAY=$(request GET /api/v1/auth/me "$TOKEN" "" "$TMP/me2" >/dev/null; jget "$TMP/me2" "data.wallet_balance" 2>/dev/null || echo "?")
    if [ "$code" = "201" ] && [ "$REPLAY_ORDER" = "$ORDER_ID" ] && [ "$BALANCE_REPLAY" = "$BALANCE_AFTER" ]; then
        green "idempotent replay: same order, balance unchanged ($BALANCE_AFTER)"
    else
        red "idempotent replay" "HTTP $code, order $REPLAY_ORDER, balance $BALANCE_REPLAY"
    fi

    code=$(request GET "/api/v1/digital/my-vault?per_page=20" "$TOKEN" "" "$TMP/vault")
    ENTITLEMENT_ID=$(jget "$TMP/vault" "data.0.entitlement_id" 2>/dev/null || true)
    if [ "$code" = "200" ] && [ -n "$ENTITLEMENT_ID" ]; then green "GET /digital/my-vault"; else red "GET /digital/my-vault" "HTTP $code"; fi

    code=$(request POST "/api/v1/digital/download/$ENTITLEMENT_ID" "$TOKEN" "" "$TMP/download")
    URL=$(jget "$TMP/download" "data.download_url" 2>/dev/null || true)
    case "$URL" in
        *X-Amz-Signature*) green "POST /digital/download -> signed URL" ;;
        *) red "POST /digital/download" "HTTP $code, no signature in URL" ;;
    esac

    code=$(request POST "/api/v1/digital/orders/$ORDER_ID/refund" "$TOKEN" "" "$TMP/refund")
    if [ "$code" = "403" ]; then
        green "refund as client -> 403"
    else
        red "refund as client" "expected 403, got $code"
    fi

    echo
    echo "  order: $ORDER_NUMBER"
else
    echo
    echo "  money paths skipped: no way to fund the wallet."
    echo "  re-run as: DATABASE_URL=postgres://postgres:postgres@localhost:5432/erp_commerce ./scripts/smoke.sh"
fi

echo
printf '  %d passed, %d failed, %d skipped\n' "$PASS" "$FAIL" "$SKIP"
[ "$FAIL" -eq 0 ] || exit 1
