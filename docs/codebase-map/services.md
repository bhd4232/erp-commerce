# Module: Agency Services

**Directory:** `src/modules/services/`  
**Tables:** `agency_services`, `service_orders`, `order_briefs`,
`order_deliverables`, `order_revisions`  
**Endpoints:** order create/list/detail, brief, assign, deliverable, revision,
approve, cancel, final download

## Files

- `models.rs` — status enum and row types.
- `dtos.rs` — order workspace, brief, deliverable, action responses.
- `service.rs` — order creation/debit, role-scoped board/detail, final download.
- `transitions.rs` — state machine; every transition locks with `FOR UPDATE`.
- `handlers.rs` — HTTP only.

## Invariants

- Status changes only through `transitions.rs`.
- Illegal transition returns `409 invalid_state_transition`.
- Only the assigned staff member or an admin uploads a deliverable.
- Client can request revisions only under `under_review` and within the limit.
- Final asset is signed only after `completed` and `is_approved`.
- Order creation debits the wallet and writes the ledger row in one transaction.
- Cancellation refunds once and is terminal.

## Tests

- `tests/service_flow.rs` — lifecycle, role scoping, presign allow-list,
  cancel/refund, revision limit.
