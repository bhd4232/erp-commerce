# Frontend & Tauri Directory Architecture (Vue 3 + TypeScript)

Companion to `UI_DESIGN_SYSTEM.md`, `UI_SCREEN_SPECS.md`, and
`UI_USER_FLOWS.md`. Screen behaviour lives there; this file is structure only.

---

## 1. Stack

| Concern | Choice |
| --- | --- |
| Framework | Vue 3.5+, `<script setup lang="ts">` only |
| Build | Vite 6 |
| Styling | Tailwind CSS v4 (CSS-first `@theme`, **no `tailwind.config.js`**) |
| State | Pinia 2 (setup stores) |
| Routing | Vue Router 4, history mode |
| HTTP | a single typed client on top of `fetch` |
| i18n | `vue-i18n` 10, Bengali default |
| Charts | none in v1 (KPI cards only) |
| Desktop/mobile | Tauri v2, see `TAURI_NATIVE_SPEC.md` |

> Tailwind v4 is configured in CSS, not JS. `tailwind.config.js` must not exist.

---

## 2. Directory Tree

```text
frontend/
├── package.json
├── vite.config.ts
├── tsconfig.json
├── index.html
├── src-tauri/                    # only when building the desktop/mobile shell
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── capabilities/
│   │   └── default.json          # Tauri v2 permissions (explicit allow-list)
│   └── src/
│       ├── main.rs
│       └── commands.rs           # save_file, open_path, notify
└── src/
    ├── main.ts
    ├── App.vue
    │
    ├── styles/
    │   ├── main.css              # @import tailwind; @theme tokens; dark/light
    │   └── fonts.css             # re-export of assets/fonts/anek-bangla/fonts.css
    │
    ├── api/
    │   ├── client.ts             # base fetch + JWT + error envelope mapping
    │   ├── auth.api.ts
    │   ├── digital.api.ts
    │   ├── services.api.ts
    │   ├── billing.api.ts
    │   ├── generate.api.ts       # + SSE helper
    │   ├── notifications.api.ts
    │   └── settings.api.ts
    │
    ├── stores/
    │   ├── auth.store.ts         # session, role, wallet, ai_credits
    │   ├── vault.store.ts        # entitlements + download counters
    │   ├── pipeline.store.ts     # orders, kanban columns, filters
    │   ├── billing.store.ts      # invoices, top-up state
    │   └── ui.store.ts           # theme, sidebar, toast queue
    │
    ├── composables/
    │   ├── useAsync.ts           # loading / error / data for one call
    │   ├── usePagination.ts
    │   ├── useSlaTimer.ts
    │   ├── useMoney.ts           # Intl.NumberFormat wrapper
    │   ├── useToast.ts
    │   └── useConfirm.ts         # promise-based confirmation dialog
    │
    ├── layouts/
    │   ├── AppLayout.vue         # sidebar + topbar + <RouterView>
    │   ├── AuthLayout.vue
    │   └── SettingsLayout.vue
    │
    ├── components/
    │   ├── ui/                   # atoms only, no business logic
    │   │   ├── AppButton.vue
    │   │   ├── StatusBadge.vue
    │   │   ├── DataTable.vue
    │   │   ├── ModalSheet.vue
    │   │   ├── ConfirmDialog.vue
    │   │   ├── FileUploadDropzone.vue
    │   │   ├── MoneyText.vue
    │   │   ├── EmptyState.vue
    │   │   ├── ToastHost.vue
    │   │   └── Timeline.vue
    │   ├── kanban/
    │   │   ├── KanbanBoard.vue
    │   │   ├── KanbanColumn.vue
    │   │   └── OrderCard.vue
    │   ├── order/
    │   │   ├── ProofPlayer.vue
    │   │   ├── RevisionModal.vue
    │   │   ├── ApproveDialog.vue
    │   │   └── ActivityStream.vue
    │   ├── studio/
    │   │   ├── BannerCanvas.vue
    │   │   ├── GeneratorControls.vue
    │   │   └── CopyStream.vue
    │   └── finance/
    │       ├── WalletChip.vue
    │       └── TopUpModal.vue
    │
    ├── views/
    │   ├── auth/
    │   │   ├── LoginView.vue
    │   │   └── RegisterView.vue
    │   ├── dashboard/OverviewView.vue
    │   ├── digital/
    │   │   ├── CatalogueView.vue
    │   │   ├── MyVaultView.vue
    │   │   └── GeneratorStudioView.vue
    │   ├── services/
    │   │   ├── KanbanPipelineView.vue
    │   │   ├── OrdersTabletView.vue      # same route, table mode
    │   │   ├── ActiveBriefsView.vue
    │   │   ├── DeliverablesView.vue
    │   │   ├── OrderWorkspaceView.vue    # /services/orders/:id
    │   │   └── BriefFormView.vue         # /services/orders/:id/brief
    │   ├── finance/
    │   │   ├── BillingView.vue
    │   │   ├── TopUpView.vue
    │   │   └── InvoicesView.vue
    │   ├── settings/
    │   │   ├── ProfileView.vue
    │   │   ├── TeamView.vue
    │   │   └── ApiKeysView.vue
    │   └── errors/
    │       ├── NotFoundView.vue
    │       └── ForbiddenView.vue
    │
    └── router/
        ├── index.ts
        └── guards.ts             # auth + role + ownership
```

`assets/fonts/anek-bangla/` stays at the repository root. Vite aliases it as
`@fonts`, and `src/styles/fonts.css` re-exports it so the app has one import
point.

---

## 3. Routing

```ts
// conceptual, not literal
{ path: '/login',            meta: { public: true } }
{ path: '/',                 component: OverviewView,        meta: { roles: ['client','staff','admin'] } }
{ path: '/digital/catalogue',component: CatalogueView,       meta: { public: true } }
{ path: '/digital/vault',    component: MyVaultView,         meta: { roles: ['client','staff','admin'] } }
{ path: '/digital/tools/generator', component: GeneratorStudioView, meta: { roles: ['client','admin'] } }
{ path: '/services/orders',  component: KanbanPipelineView,  meta: { roles: ['staff','admin'] } }
{ path: '/services/orders/:id',      component: OrderWorkspaceView }
{ path: '/services/orders/:id/brief',component: BriefFormView, meta: { roles: ['client'] } }
{ path: '/services/briefs',  component: ActiveBriefsView,    meta: { roles: ['staff','admin'] } }
{ path: '/services/deliverables', component: DeliverablesView, meta: { roles: ['staff','admin'] } }
{ path: '/finance/billing',  component: BillingView }
{ path: '/finance/topup',    component: TopUpView }
{ path: '/finance/invoices', component: InvoicesView }
{ path: '/settings/profile', component: ProfileView,         meta: { roles: ['admin'] } }
{ path: '/settings/team',    component: TeamView,            meta: { roles: ['admin'] } }
{ path: '/settings/api',     component: ApiKeysView,         meta: { roles: ['admin'] } }
```

Guards:

1. No token → redirect `/login?next=<path>`.
2. Token but role not allowed → `ForbiddenView` (403), never a silent redirect.
3. Ownership is **not** checked in the router; the API returns 403/404 and the
   view renders the corresponding state.

---

## 4. API Client Contract

`src/api/client.ts` is the only place that knows about HTTP.

```ts
type Envelope<T> = { data: T; meta: PageMeta | null };
type ApiError   = { code: string; message: string; details?: unknown; status: number };

async function request<T>(path: string, init?: RequestInit): Promise<T>;
```

Responsibilities:

- Attach `Authorization: Bearer <token>` from the auth store.
- Parse the `{ error: { code, message } }` envelope into a typed `ApiError`.
- On `401`: clear the session and route to `/login?next=…`.
- On `429`: read `Retry-After` and surface it to the caller.
- Never log request bodies containing tokens.
- No component may call `fetch` directly.

---

## 5. State Ownership

| Store | Owns | Never holds |
| --- | --- | --- |
| `auth` | token, user profile, role, wallet, ai_credits | order lists |
| `vault` | entitlements, download counters | signed URLs |
| `pipeline` | orders, kanban columns, filters, optimistic moves | payment state |
| `billing` | invoices, top-up draft | balances (mirrors `auth`) |
| `ui` | theme, sidebar, toasts, modal stack | server data |

Rules:

- Server data is refreshed by the view that needs it; stores cache but do not
  poll globally.
- **Signed URLs are never stored.** They are opened immediately and discarded.
- The wallet balance lives only in `auth.store`; `billing` reads it, never
  duplicates it.

---

## 6. Styling Rules

`src/styles/main.css`:

```css
@import "tailwindcss";
@import "./fonts.css";

@theme {
  --font-sans: "Anek Bangla Variable", "Inter", system-ui, sans-serif;
  --font-bangla: "Anek Bangla Variable", "Noto Sans Bengali", sans-serif;
  --color-bg-base: var(--bg-base);
  /* …one entry per token in UI_DESIGN_SYSTEM.md §3 */
}

:root { /* dark values */ }
[data-theme="light"] { /* light values */ }
```

- Components use `var(--token)` or the generated Tailwind class; never a hex.
- Theme switch sets `data-theme` on `<html>` and persists in `localStorage`
  keyed `theme`.
- Default is dark when the key is absent.

---

## 7. Build & Dev

```bash
npm ci
npm run dev        # vite, proxies /api -> localhost:8080
npm run build      # typecheck + bundle
npm run lint       # eslint + vue-tsc
npm run tauri:dev  # desktop shell
```

`vite.config.ts` proxies `/api` to `http://localhost:8080` so the SPA never
needs CORS in development.

---

## 8. Non-Goals for v1

- Server-side rendering.
- Multi-workspace switching (single workspace; `users.role` is the only tenancy
  axis).
- Offline writes (offline is read-only cache, see `TAURI_NATIVE_SPEC.md`).
- Component library extraction to a separate package.
