# Rule: Money and ledger

**Tier:** 1. A mistake here loses money or breaks reconciliation.

## Wallet

- `users.wallet_balance` is a cached projection; `wallet_transactions` is the
  source of truth.
- Every credit/debit/refund/adjustment updates the cached balance and inserts
  the matching ledger row in one transaction.
- `wallet_transactions.amount` is always positive; direction comes from
  `txn_type`.
- `balance_after` is the snapshot after applying the row.
- Never update a cached balance without a ledger row, and never insert a ledger
  row without updating the balance.
- A gateway transaction can credit once: `(gateway, gateway_ref)` is unique when
  present.
- A replayed webhook changes no balance and writes no second ledger row.
- Refunds write one `refund` row and are idempotent by business reference.

## AI credits

- `users.ai_credits` is a cached projection; `ai_credit_transactions` is the
  ledger.
- A job debit and the job row share one transaction; a terminal failure writes
  one refund row in one transaction.

## Presentation

- Money is `NUMERIC(12,2)` in the database and a decimal string in JSON; never a
  float.
- Never return `gateway_ref` or a storage key in a DTO.
- Tests assert the ledger and cached balance move together and roll back
  together.
