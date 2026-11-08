# User Interaction Flows & Edge States

Companion to `UI_SCREEN_SPECS.md`. Every flow names the API call, the UI state
before and after, and what happens when the call fails. The client never
invents a transition the backend did not confirm.

---

## 1. Client Service Onboarding

```
Service detail
  └─ "Order now"
      └─ Checkout modal
          ├─ wallet sufficient → POST /services/orders
          │     └─ 201 → route /services/orders/:id/brief
          └─ 402 insufficient_funds → "Add funds" link (balance unchanged)
```

1. Client opens a service and clicks **Order now**.
2. Checkout modal shows price, current balance, balance after.
3. Confirm opens a blocking loading state on the button only — the modal stays
   interactive-until-busy, then locks.
4. On `201`, close the modal and navigate to the brief form. The order exists
   with status `pending_brief`.
5. The brief form shows the SLA banner: *"Your SLA countdown begins only after
   brief submission."*
6. Submit → `POST /services/orders/{id}/brief` → status `in_queue`.
7. Success toast: "Brief submitted. Your order is in the queue."
8. Route to the Order Hub; the timeline advances to step 2.

**Edge cases**

| Event | UI |
| --- | --- |
| Double-click Confirm | Button disabled on first click; `idempotency_key` sent |
| Modal closed mid-request | Request continues, result handled by a global toast |
| `422 brief_incomplete` | Missing fields highlighted, first one focused |
| `409 invalid_state_transition` | Hub reloaded; toast "This order already moved on." |
| Network drop after submit | Retry with the same payload; server dedupes on status |

---

## 2. Proof Review & Revision

### 2.1 Request Revision

```
"Request Revision"
  ├─ revisions_used >= revision_limit
  │     └─ toast (persistent): free revisions exhausted → add-on CTA
  └─ feedback.length < 20
        └─ inline error, submit disabled
  └─ valid
        └─ POST .../revision → revision_requested
```

- The modal shows a live character counter (`n / 20`) and disables submit below
  20 characters.
- On success the status banner flips to **Revision**, the timeline returns to
  step 3, and the activity stream appends the feedback.
- If the backend still returns `409 revision_limit_reached` (client state was
  stale), the modal closes and the persistent add-on toast appears. Do not
  retry automatically.

### 2.2 Approve Delivery

1. Client clicks **Approve Delivery**.
2. Confirmation dialog: *"Approving will finalize this order and unlock raw
   production files. This cannot be undone."* — primary button reads
   "Approve & unlock".
3. Confirm → `POST .../approve`.
4. On success:
   - status → `completed`, timeline step 5
   - confetti fires once (≤ 1.2s, skipped under reduced motion)
   - the watermarked player is replaced by a green **Download Source Package
     (.ZIP)** CTA
   - activity stream appends "Client approved delivery"
5. Download CTA → `GET .../deliverable/{id}/download` → signed URL opened in a
   new tab (900 s TTL).

**Edge cases**

| Event | UI |
| --- | --- |
| Preview still `processing` | Approve disabled with "Preview is still being prepared" |
| `409` on approve | Refresh hub, toast, no confetti |
| `410` on download | "This link expired" + Regenerate button |
| `403` on download | Toast "Not available yet"; re-fetch order |

---

## 3. Digital Purchase & Download

```
Add to cart / Buy
  └─ POST /digital/checkout (idempotency_key)
      ├─ 201 paid → entitlement created
      └─ 402 → top-up modal
Vault → Download
  └─ POST /digital/download/{id}
      ├─ 200 → window.open(download_url)
      ├─ 410 entitlement_expired → card pill "Expired"
      ├─ 410 downloads_exhausted → button disabled "Limit reached"
      └─ 403 → toast (should be unreachable; log it)
```

- The download button shows `downloading…` until the URL returns, then resets.
- The URL is never rendered in the DOM, never copied to the clipboard
  automatically, and never stored in Pinia.
- After a successful download, increment the local counter optimistically and
  reconcile with the next vault fetch.
- Retrying `checkout` with the same `idempotency_key` must not create a second
  entitlement; the UI treats the replayed response as success.

---

## 4. Kanban Drag & Drop

Allowed transitions only. Everything else is rejected client-side **and**
returns `409` server-side.

| From column | Allowed drop targets | Actor |
| --- | --- | --- |
| Brief Pending | Queue | system only — **not draggable** |
| Queue | In Progress | staff/admin (triggers assign) |
| In Progress | Client Review | assigned staff/admin |
| Client Review | Completed | client only (via Approve, not drag) |
| Revision | In Progress | assigned staff/admin |

- Cards in `Brief Pending` and `Client Review` are not draggable by staff.
- Client approval is never a drag; it must go through the confirmation dialog.
- On a valid drop: optimistic move + `PATCH .../assign` or the relevant action.
- On failure: card animates back to the origin, toast explains why, board
  refetches.
- **Keyboard**: each card has a "Move to…" menu (Space opens it), listing only
  valid targets. This is the accessible equivalent of dragging and is required,
  not optional.

---

## 5. API Error → UI Reaction

| HTTP | Code | UI response |
| --- | --- | --- |
| 400 | `validation_error` | Field-level errors from `details`; focus first invalid field |
| 401 | `unauthorized` | Clear session, route to `/login?next=…` |
| 402 | `insufficient_funds` | Inline warning + "Top up wallet" modal; balance unchanged |
| 403 | `forbidden` | Toast + disable the action; never a blank page |
| 404 | `not_found` | Screen-level not-found state with a "Back" action |
| 409 | `invalid_state_transition` | Refetch the resource, toast "This item already moved on" |
| 409 | `revision_limit_reached` | Persistent toast + add-on CTA |
| 409 | `email_taken` | Field-level error on the email input |
| 410 | `entitlement_expired` | Card pill → Expired; download disabled |
| 410 | `downloads_exhausted` | Button → "Limit reached"; disabled |
| 422 | `brief_incomplete` | Highlight missing brief fields |
| 429 | `rate_limited` | Toast with retry-after; back off, disable submit |
| 500 | `internal_error` | Generic toast with request id; Retry for idempotent GETs |
| 501 | `not_implemented` | "Coming soon" inline state (dev only) |

Rule: **never** surface a raw backend message. Map by `code`, log the `message`
with the request id.

---

## 6. Optimistic Updates

Use optimistic UI only where rollback is invisible:

| Action | Optimistic? | Rollback |
| --- | --- | --- |
| Kanban move | yes | animate card back + refetch |
| Download counter | yes | reconcile on next vault fetch |
| Mark notification read | yes | silent revert |
| Approve / reject order | **no** | server confirms first |
| Checkout / payment | **no** | server confirms first |
| Wallet top-up | **no** | server confirms first |

Payments and approvals are never optimistic — a wrong confirmation is worse
than a spinner.

---

## 7. Wallet & Top-up Flow

1. Low-balance chip turns warning when `balance < 25` (configurable).
2. Top-up modal: amount presets + custom, currency selector (USD default, BDT
   supported), fee line, resulting balance.
3. Submit is disabled while invalid or zero.
4. Success → chip updates, toast, modal closes.
5. Failure → inline error, modal stays open, amount preserved.

---

## 8. Responsive & Tauri

| Context | Behaviour |
| --- | --- |
| `< 768px` | Sidebar → bottom nav; kanban → swipeable tabs (one status per tab); preview → full-screen sheet |
| Tablet | 2-column grids; kanban horizontally scrolls with sticky column headers |
| Desktop app (Tauri) | Same layout; native window controls offset the top bar |
| Offline | Persistent top banner "You're offline"; reads served from cache are marked "cached"; writes disabled |
| Reconnect | Banner clears, background refetch, queued drafts preserved |
| Deep link | `erp://orders/{id}` focuses the window and routes to the hub |

---

## 9. Accessibility Flows

- Every modal: focus enters the first field, `Esc` closes (unless busy), focus
  returns to the trigger.
- Toasts: `aria-live="polite"`, except errors which use `role="alert"`.
- Status changes made by another user are announced through a live region only
  when the user is on that order — not globally.
- Upload progress has `aria-valuenow` on the progress element.
- Colour is never the only signal; every status has a text label.

---

## 10. Analytics Events (names are part of the contract)

```
auth.login_succeeded        auth.login_failed
order.created               order.brief_submitted
order.assigned              order.deliverable_submitted
order.revision_requested    order.approved
order.cancelled
download.granted            download.denied   (reason: expired|limit|forbidden)
checkout.started            checkout.completed  checkout.failed
wallet.topup_started        wallet.topup_completed
```

Properties: `order_id`, `product_id`, `status_from`, `status_to`, `role`,
`request_id`. Never send PII, tokens, or signed URLs.
