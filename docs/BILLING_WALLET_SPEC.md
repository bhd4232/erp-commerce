# Billing, Credit Wallet & Payment Gateway Specification

Extends `DATABASE.md` §2.1 and `API_SPEC.md` §3. This document is normative for
money. If a code path moves funds without this file's rules, it is a bug.

---

## 1. Two Balances, Two Ledgers

Every client has two independent balances. They never convert into each other
implicitly.

| Balance | Column | Unit | Funded by | Spent on |
| --- | --- | --- | --- | --- |
| Cash | `users.wallet_balance` | currency minor-unit decimal | top-up, refunds | digital products, service orders |
| AI credits | `users.ai_credits` | integer | purchase, promo grant, refund | generation jobs |

Each is a **cached projection**. The ledger is the source of truth:

- `wallet_transactions` (table created in `0001_init_schema.sql`)
- `ai_credit_transactions` (table created in `0003_wallet_credits.sql`)

Invariant: for every user, `users.wallet_balance` equals the derived balance of
`wallet_transactions`, and `users.ai_credits` equals the derived balance of
`ai_credit_transactions`. A reconciliation job asserts this daily.

---

## 2. Existing Wallet Ledger — Do Not Duplicate

`0001` already defines `wallet_transactions`. The shape is fixed:

```sql
txn_type      wallet_txn_type   -- 'credit' | 'debit' | 'refund' | 'adjustment'
amount        NUMERIC(12,2)     -- always POSITIVE; direction comes from txn_type
balance_after NUMERIC(12,2)     -- snapshot after applying this row
reference_type VARCHAR(50)      -- 'digital_order' | 'service_order' | 'topup' | 'manual'
reference_id  UUID
created_at    TIMESTAMPTZ
```

`0003` adds the gateway columns without changing that contract:

```sql
gateway      VARCHAR(50)   -- 'bkash' | 'sslcommerz' | 'stripe' | 'internal'
gateway_ref  VARCHAR(255)  -- gateway transaction id
currency     CHAR(3)       -- ISO 4217, default 'USD'
```

> Do **not** create a second `wallet_transactions` table or switch `amount` to a
> signed value. Older rows exist; a signed amount would silently invert every
> historical debit.

### 2.1 Direction mapping

| Business event | `txn_type` | Amount sign in code | Ledger row |
| --- | --- | --- | --- |
| Wallet top-up confirmed | `credit` | positive | `+amount` |
| Buy digital product | `debit` | positive | `-amount` |
| Pay for service order | `debit` | positive | `-amount` |
| Refund / cancellation | `refund` | positive | `+amount` |
| Manual correction | `adjustment` | positive | `±` per `note` |

`reference_type` carries the business meaning that the proposed
`deposit/purchase_digital/purchase_service` enum would have carried. Do not
add a competing enum.

---

## 3. AI Credit Ledger

```sql
ai_credit_txn_type  -- 'grant' | 'topup' | 'spend' | 'refund' | 'adjustment'
credits             -- always positive
balance_after       -- snapshot
reference_type      -- 'generation_job' | 'purchase' | 'manual'
```

Rules:

- Credits are integers. Never fractional; never store a credit as money.
- A price table maps a generation type to a credit cost
  (`banner_1080 = 2`, `ad_copy = 1`, `packing_slip = 1`). Prices live in config,
  not in code branches.
- A failed generation **must** produce a `refund` row in the same transaction
  that marks the job failed.
- Promo grants use `grant` and must carry a `note`.

---

## 4. Transaction Contract

Every mutation follows this shape, inside one `sqlx::Transaction`:

```rust
// 1. Debit the cached balance atomically. Zero rows = insufficient funds.
let balance_after: Decimal = sqlx::query_scalar(
    "UPDATE users SET wallet_balance = wallet_balance - $1
      WHERE id = $2 AND wallet_balance >= $1
      RETURNING wallet_balance"
).bind(amount).bind(user_id).fetch_one(&mut *tx).await?;

// 2. Insert the ledger row with the returned snapshot.
// 3. Insert the business row (order / entitlement / invoice).
// 4. Commit. Any error rolls all three back.
```

Never `SELECT balance` then `UPDATE`. The check and the write are one statement.

### 4.1 Credits

```sql
UPDATE users SET ai_credits = ai_credits - $1
 WHERE id = $2 AND ai_credits >= $1
RETURNING ai_credits;
```

Zero rows → `402 insufficient_credits` (see `API_SPEC.md`).

---

## 5. Payment Gateways

### 5.1 Providers

| Provider | Region | Currency | Notes |
| --- | --- | --- | --- |
| `bkash` | BD | BDT | Tokenised checkout, requires callback URL |
| `sslcommerz` | BD | BDT | Hosted page, IPN + success/fail/cancel URLs |
| `stripe` | global | USD | Payment Intents + signed webhooks |
| `wallet` | — | internal | No external call; instant settlement |
| `card` | global | USD | Legacy/manual card capture, kept from `0001` |
| `manual` | — | any | Admin-recorded settlement (bank transfer, cash) |

Two columns record the provider, for two different jobs:

- `payments.provider` — the `payment_provider` enum, extended in `0003` with
  `bkash`, `sslcommerz`, and `stripe`. This is *how the order was paid*.
- `wallet_transactions.gateway` — free-text provenance on the ledger row
  (`'bkash'`, `'sslcommerz'`, `'stripe'`, `'internal'`, `'manual'`). This is
  *how the wallet was funded*.

Do not add a third enum for gateways.

Amounts are always sent in **minor units** to the gateway and stored as
`NUMERIC(12,2)` internally. Never send a float.

### 5.2 Top-up flow

```
POST /finance/topup
  └─ create payment_intent row (status pending)
      └─ return gateway redirect / client secret
          └─ gateway callback/webhook
              └─ verify signature
                  └─ idempotent credit (see 5.3)
                      └─ mark intent paid, issue invoice
```

### 5.3 Webhook idempotency

`payment_webhooks` (table in `0003`) has `UNIQUE (provider, event_id)`.
Processing order:

1. Insert the raw event. A duplicate `(provider, event_id)` → return `200` and
   stop (already seen).
2. Verify the signature over the **raw body**, not the parsed JSON. On failure
   set `signature_valid = false`, `status = 'rejected'`, return `400`.
3. Apply the credit inside a transaction.
4. Set `status = 'processed'`, `processed_at = NOW()`.

A second defence sits in the wallet itself: `UNIQUE (gateway, gateway_ref)`
rejects a double credit even if two different event ids describe the same
transaction.

**Webhooks must be idempotent and order-independent.** Never trust the arrival
order of `paid` and `refunded`.

### 5.4 Refund policy

- Digital product: refundable only if the entitlement has zero downloads and
  the request is within 24 h.
- Service order: refundable before `in_progress`; after that, prorated by admin
  decision and logged as `adjustment` with a reason.
- Refunds always create a `refund` ledger row; never mutate the original debit.

---

## 6. Invoices

`invoices` (table in `0004`) is append-only.

- `invoice_number` format: `INV-YYYYMM-000001`, generated from a Postgres
  sequence per month.
- One invoice per paid top-up and per paid order.
- `status` transitions: `draft → open → paid` or `open → void` /
  `paid → refunded`. Never delete.
- PDFs are rendered to storage and served through a pre-signed URL with a
  900 s TTL. The `pdf_path` never leaves the server.
- Tax is stored separately (`tax_amount`) so the invoice can be re-rendered.

---

## 7. Currency

- Workspace default: **USD**. BDT is fully supported per user locale.
- A user's transactions are single-currency; cross-currency conversion is out
  of scope for v1.
- Display formatting is the frontend's job (`<MoneyText>` + `Intl.NumberFormat`).
  The backend returns a decimal string, never a formatted string.
- `৳` (U+09F3) renders from the self-hosted Anek Bangla Bengali subset.

---

## 8. Reconciliation Job

Daily, per user and in aggregate:

```sql
-- wallet
SELECT u.id,
       u.wallet_balance,
       COALESCE(SUM(CASE WHEN w.txn_type IN ('credit','refund','adjustment')
                         THEN w.amount ELSE -w.amount END), 0) AS derived
  FROM users u LEFT JOIN wallet_transactions w ON w.user_id = u.id
 GROUP BY u.id, u.wallet_balance
HAVING u.wallet_balance <> COALESCE(SUM(...), 0);
```

Any row returned is an incident: alert, freeze that wallet, investigate. Never
"fix" a balance without an `adjustment` ledger row.

---

## 9. Failure Modes

| Failure | Behaviour |
| --- | --- |
| Gateway timeout after charge | Do not credit. Wait for webhook; reconcile via gateway report. |
| Webhook signature invalid | Reject, log, alert. Never credit. |
| Duplicate webhook | `200`, no ledger change. |
| Credit succeeded, invoice insert failed | Whole transaction rolls back; webhook retried. |
| AI job fails after credit spend | `refund` row + `402`-free retry path |
| Balance drift detected | Freeze wallet, require admin `adjustment` |
