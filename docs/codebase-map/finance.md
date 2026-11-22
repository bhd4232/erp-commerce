# Module: Finance

**Directory:** `src/modules/finance/`  
**Tables:** `wallet_transactions`, `ai_credit_transactions`,
`payment_webhooks`, `topup_intents`, `invoices`, `notifications`  
**Endpoints:** wallet, transactions, top-up, invoices, invoice detail,
`POST /webhooks/payment/{provider}`

## Files

- `dtos.rs` — wallet/transaction/top-up/invoice projections.
- `service.rs` — wallet reads, top-up intent, invoice list/detail.
- `webhooks.rs` — signature verification, idempotency, ledger credit + invoice.
- `handlers.rs` — HTTP only.

## Invariants

- `users.wallet_balance` is a cached projection; the ledger is the source of
  truth.
- A credit/debit/refund updates the cached balance and inserts the matching
  ledger row in one transaction.
- `wallet_transactions.amount` is always positive; direction comes from
  `txn_type`.
- A replayed `(provider, event_id)` returns `200` and changes nothing.
- A bad signature returns `400` and records a rejected webhook.
- Gateway references are never returned in a DTO.
- Invoice PDFs are signed only when `pdf_path` is present.

## Tests

- `tests/finance_flow.rs` — top-up validation, webhook idempotency, bad
  signature, wallet/transaction/invoice reads.
