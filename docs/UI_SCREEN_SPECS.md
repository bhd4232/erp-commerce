# Detailed Screen Specifications & Wireframe Layouts

Every screen below is specified with: route, role access, layout grid, sections,
data source (exact API call), and the four required states
(**loading / empty / error / disabled**). If a screen is not in this file it is
not in scope.

---

## 1. Global Shell (`AppLayout.vue`)

### 1.1 Top bar — 56px, sticky, `z-30`, `--bg-surface`, 1px bottom border

| Slot | Content | Behaviour |
| --- | --- | --- |
| Left | Product logo + workspace selector | `Agency Services` / `Digital Vault`; switching swaps sidebar groups, not the route |
| Center | Global search (`⌘K`) | Orders by number, products by title, clients by email |
| Right | Wallet chip → `MoneyText` | Click opens Wallet Top-up; low-balance state below threshold |
| Right | Notification bell + unread dot | Dropdown lists status changes, `aria-label="Notifications, N unread"` |
| Right | Avatar → menu | Profile, Settings, Theme toggle, Sign out |

### 1.2 Sidebar — 240px, collapsible to 64px

| Group | Items |
| --- | --- |
| Core | Dashboard |
| Digital Studio | Digital Assets Catalogue, My Vault, AI Creative Generator |
| Agency Pipeline | Orders, Active Briefs, Deliverables |
| Finance | Billing, Wallet Top-up, Invoices |
| Settings | Workspace Profile, Team Members, API/Webhooks |

- Active item: `--primary-soft` background, `--primary` left bar, `--fg` text.
- Role filter: `client` sees Core, Digital Studio, My Orders, Finance (own);
  `staff` sees Core, Agency Pipeline, Deliverables; `admin` sees everything plus
  Settings.
- `< 768px`: sidebar collapses to a 64px bottom nav with the 5 most-used
  destinations; the rest move into "More".

### 1.3 Content region

Max width 1440px, page gutter `space-4` / `space-8`, vertical stack with
`space-6` between sections. Page header pattern:

```
H1 + optional meta line (order number, dates)
                                    [secondary action] [primary action]
```

---

## 2. Canonical Status Mapping

Single source for labels, colours, and kanban columns. Do not translate status
strings ad hoc.

| DB status | UI label | Column | Tone |
| --- | --- | --- | --- |
| `pending_brief` | Brief Pending | 1 | warning |
| `in_queue` | Queue | 2 | neutral |
| `in_progress` | In Progress | 3 | info |
| `under_review` | Client Review | 4 | warning |
| `revision_requested` | Revision | 3 (returns to In Progress) | danger |
| `completed` | Completed | 5 | success |
| `cancelled` | Cancelled | hidden from board | neutral |

`approved` is not rendered as a status. Approval is `completed` plus
`deliverable.is_approved === true`.

---

## 3. Auth Screens

### Login — `/login`

- Centered card 400px on `--bg-base`, brand mark above.
- Fields: email, password (show/hide toggle), "Remember me", "Forgot password".
- Primary `Sign in`; secondary link to Register.
- States: `loading` (button spinner, fields disabled), `error` (`401` →
  inline "Invalid email or password", never reveal which), `disabled` while
  submitting. Rate limit `429` → "Too many attempts. Try again in N minutes."
- Data: `POST /api/v1/auth/login`.

### Register — `/register`

- Same shell. Fields: full name, email, password (min 12), confirm password.
- Inline validation on blur, not on every keystroke.
- `409 email_taken` → field-level error on email.
- Data: `POST /api/v1/auth/register`, then `POST /login` or redirect to login.

---

## 4. Core

### Dashboard — `/`

- **KPI row** (4 cards): Open orders, Awaiting my review, Wallet balance,
  Downloads this month. Each card: label (`micro`), value (`h2`, tabular),
  delta vs previous period.
- **Pipeline snapshot**: horizontal counts per status, click → filtered Orders.
- **Recent activity**: last 10 audit/status events, relative timestamps.
- **Attention list**: overdue orders (SLA < 0) and orders in `under_review` for
  > 7 days.
- Data: `GET /services/orders?per_page=100` + `GET /auth/me`.
- States: skeleton cards (fixed height); empty → "No orders yet" with
  "Browse services"; error → inline retry per card, not a blank page.

---

## 5. Digital Studio

### 5.1 Digital Assets Catalogue — `/digital/catalogue`

- Header: title + type filter chips (`All`, `Templates`, `Ad Packs`, `Scripts`,
  `Tools`).
- Grid: 1 / 2 / 3 / 4 columns at `sm` / `md` / `lg` / `xl`.
- Card: thumbnail 16:9 with skeleton, title, `product_type` badge, `MoneyText`
  price, `v2.1` version tag, "Owned" pill when entitled.
- Card click → detail drawer: description, file metadata (size, mime, version),
  primary `Buy` or `Download`.
- Data: `GET /digital/products`, `GET /digital/products/{slug}`.
- States: grid skeleton (8 cards), empty filter → "No products in this
  category", error → retry banner.

### 5.2 My Vault — `/digital/vault`

- Header stats: Total purchased, Downloads remaining (sum, `∞` when `-1`),
  Storage used.
- Search + type filter + sort (recent, title).
- Asset card: thumbnail, version tag, title, purchase date, licence type,
  `Download Asset` primary, `Changelog` secondary (modal).
- Download click: button enters `loading`, calls
  `POST /digital/download/{entitlement_id}`, then opens the returned
  `download_url` in a new tab. Never render the URL in the page.
- Per-card limit state: `max_downloads !== -1 &&
  download_count >= max_downloads` → button `disabled` with "Limit reached".
  Expired → "Expired" pill.
- Data: `GET /digital/my-vault`.
- States: skeleton rows; empty → "Your vault is empty" + "Browse catalogue";
  `410` → toast + card moves to exhausted; `403` → toast (should not happen).

### 5.3 AI Creative Generator — `/digital/tools/generator`

- Split screen 40 / 60, stacking vertically below `lg`.
- **Left panel**: product title, discount %, aspect ratio (1:1, 9:16), PNG
  upload (dropzone, transparent PNG only, ≤ 10 MB), "Generate copy" button
  with streaming state, tone selector.
- **Right canvas**: live preview, zoom control, export menu (PNG, WebP, PDF
  packing slip).
- Streaming: text appears token by token; a "Stop" button replaces "Generate".
  Do not block the canvas while streaming.
- States: initial empty canvas illustration; generating (disabled export);
  generation failed → retry with the same inputs preserved.
- Data: `POST /digital/generate` and `GET /digital/generate/{job_id}` (see
  `API_SPEC.md` §7 and `AI_GENERATOR_PIPELINE.md`); uploads use the pre-signed
  PUT flow in `API_SPEC.md` §6.

---

## 6. Agency Pipeline

### 6.1 Orders — `/services/orders`

- View toggle: **Kanban** (default for staff/admin) and **Table** (default for
  client).
- Filters: status, assignee (`me`), service, date range, search by order number.
- Kanban:
  - 5 columns from §2. Column header: label, count, "+" for admins.
  - Column body scrolls independently; page does not.
  - Card: order number, client, service title, `SlaTimer`, assignee avatar,
    unread badge.
  - Drag rules in `UI_USER_FLOWS.md` §4. Invalid drop shakes once and does
    nothing.
- Table columns: Order, Client, Service, Status, Assignee, Due, Price, Updated.
  Sticky header, sortable, paginated 20/page.
- Data: `GET /services/orders`.
- States: column skeleton (3 cards each), empty column → dashed placeholder,
  error → retry at board level; `409` after a drag → refresh + toast.

### 6.2 Active Briefs — `/services/briefs`

- List of orders in `pending_brief` and recently submitted briefs.
- Row: order, client, service, submitted at, completeness (validated fields /
  total).
- Click → brief detail drawer with the raw `payload` rendered as a key/value
  list plus uploaded asset thumbnails (signed URLs, short-lived).
- Actions: nudge client (sends notification), open order.
- Data: `GET /services/orders?status=pending_brief`, `GET /services/orders/{id}`.
- States: empty → "No briefs awaiting submission"; error → retry.

### 6.3 Deliverables — `/services/deliverables`

- Gallery of the latest deliverable per active order.
- Card: order, version, watermark status (`processing` / `ready` / `failed`),
  uploaded by, uploaded at.
- `processing` shows a progress indicator and disables preview.
- `failed` shows a danger banner with "Retry watermark" (admin).
- Data: derived from `GET /services/orders/{id}` for each active order.
- States: skeleton tiles; empty → "Nothing delivered yet".

### 6.4 Order Hub (client) — `/services/orders/:id`

1. **Status banner**: `StatusBadge` + 5-step timeline
   (Brief → Queue → In Progress → Review → Completed). `cancelled` replaces the
   timeline with a terminal banner and the reason.
2. **Order details & brief**: collapsible card; brief payload as key/value;
   attachments as thumbnails.
3. **Proofing player**: embedded image/video viewer. Watermarked preview only.
   Download is disabled until `completed`. Right-click and native video
   download are blocked for previews.
4. **Action bar** (renders only what `allowed_actions` returns):
   - `revision` → modal, min 20 characters, counter, submit disabled until met.
   - `approve` → confirmation dialog naming what unlocks, then approval.
5. **Activity stream**: threaded system events + direct chat, oldest first,
   composer pinned at the bottom.

- Data: `GET /services/orders/{id}`.
- States: page skeleton; `403` → "You do not have access"; `404` → not-found
  page; deliverable `processing` → "Preparing your preview" with polling.

### 6.5 Brief Form — `/services/orders/:id/brief`

- Two-column: form (left) + guidance panel (right, collapsible on mobile).
- Banner: "Your SLA countdown begins only after brief submission."
- Fields generated from `agency_services.brief_template` — text, textarea,
  select, file. Required markers from the template.
- Uploads: `FileUploadDropzone` → pre-signed PUT → key included in
  `uploaded_asset_keys`.
- Submit: validates all required fields, calls `POST .../brief`, then routes to
  the Order Hub with a success toast.
- States: autosave draft to local storage; `422 brief_incomplete` highlights
  missing fields; `409` → "Brief already submitted" and route to the hub.

### 6.6 Checkout modal

- Opened from a service detail page; `ModalSheet` center, size `md`.
- Summary: service, price, wallet balance, balance after.
- Payment: wallet (default) — disabled with "Add funds" when insufficient.
- `Confirm` → `POST /services/orders`; success closes the modal and routes to
  the brief form.
- `402 insufficient_funds` → inline warning + "Top up wallet" link.

---

## 7. Finance

### 7.1 Billing — `/finance/billing`

- Current plan, billing cycle, next invoice date.
- Payment methods list with default badge; add/remove.
- Invoice history (last 5) with a link to Invoices.
- States: empty payment methods → add CTA; error → retry per section.

### 7.2 Wallet Top-up — `/finance/topup`

- Balance hero (`MoneyText`, large, tabular).
- Amount presets (10 / 25 / 50 / 100) + custom input, currency selector.
- Summary with fee line (if any) and resulting balance.
- Primary `Add funds` → payment flow; loading state blocks double submit.
- States: `402`/declined → inline error, balance unchanged; success → toast +
  updated chip in the top bar.

### 7.3 Invoices — `/finance/invoices`

- `DataTable`: Invoice #, Date, Amount, Status, Download.
- Status uses the order-status tones (`paid`, `pending`, `refunded`).
- Download → signed URL, short-lived, opens in a new tab.
- States: skeleton rows; empty → "No invoices yet"; error → retry.

---

## 8. Settings

### 8.1 Workspace Profile — `/settings/profile`

- Logo upload, workspace name, default currency (USD default, BDT available),
  locale, timezone.
- Danger zone: transfer ownership, delete workspace (typed confirmation).

### 8.2 Team Members — `/settings/team`

- `DataTable`: Name, Email, Role, Status, Last active, Actions.
- Invite modal: email + role (`staff` / `admin`). Role change and removal are
  admin-only.
- States: empty → "Invite your first teammate"; pending invites shown with a
  "Resend" action.

### 8.3 API / Webhooks — `/settings/api`

- API keys table: name, prefix, created, last used, revoke.
- Create key modal shows the secret **once** with a copy button and a warning.
- Webhooks: endpoint URL, events checklist, secret, delivery log (last 20).
- States: no keys → CTA; failed deliveries highlighted with the response code.

---

## 9. Responsive Rules

| Breakpoint | Behaviour |
| --- | --- |
| `≥1280px` | Full sidebar, 4-column grids, split-screen generator |
| `768–1279px` | Icon sidebar (64px), 2–3 column grids |
| `<768px` | Bottom nav, single column, kanban → swipeable status tabs, preview → full-screen sheet, tables → card list |

Touch targets ≥ 44px. No horizontal scrolling for primary content. Modals
become bottom sheets with a drag handle.

---

## 10. State Checklist (applies to every screen)

| State | Requirement |
| --- | --- |
| Loading | Skeleton matching final layout; reserve height; no spinner-only pages |
| Empty | Icon + one-line explanation + a single next action |
| Error | Inline panel with the message and a Retry button; keep the page shell |
| Disabled | Explain *why* next to the control, not only via opacity |
| Partial | One failed section must not blank the whole page |
| Offline (Tauri) | Persistent banner + retry; cached read-only data allowed |
