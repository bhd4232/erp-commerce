# UI Design System & Tokens Specification

Implementation target: Vue 3 + Vite + Tailwind CSS v4. This document is the
single source of truth for colour, type, spacing, and component behaviour.
Anything not defined here must not be invented per screen.

---

## 1. Visual Theme & Philosophy

- **Theme**: modern B2B SaaS, clean minimalist ERP.
- **Mode**: dark-first (long working sessions, low eye fatigue), full light
  variant. Both are first-class; neither is a filter over the other.
- **Density**: comfortable by default, `compact` variant for tables and kanban.
- **Principle**: the UI never guesses state. If the backend does not expose it
  (`allowed_actions`, `status`), the button is not rendered.

---

## 2. Typography

### 2.1 Font family

**Anek Bangla Variable**, self-hosted. Files, hashes, and update steps live in
[`assets/fonts/anek-bangla/README.md`](../assets/fonts/anek-bangla/README.md).
Stylesheet: `assets/fonts/anek-bangla/fonts.css`.

| Token | Value |
| --- | --- |
| `--font-sans` | `"Anek Bangla Variable", "Inter", system-ui, -apple-system, "Segoe UI", Roboto, sans-serif` |
| `--font-bangla` | `"Anek Bangla Variable", "Noto Sans Bengali", sans-serif` |
| `--font-mono` | `"JetBrains Mono", ui-monospace, SFMono-Regular, Menlo, monospace` |

Rules:

- Anek Bangla leads the stack for **both** Bengali and Latin. There is no
  separate Latin font in the body stack; mixing families on one line breaks
  baseline alignment.
- **Never** call Google Fonts or any CDN. The font is a project asset.
- Preload only the Bengali subset (see the font README).
- Bengali line height must be ≥ 1.6. Use `--leading-bangla: 1.75` for prose.
  Bengali headings need `line-height: 1.4` minimum — descenders and *matras*
  clip at 1.0.
- Numerals in tables and money use `font-variant-numeric: tabular-nums`.

### 2.2 Type scale

| Token | Size / line-height | Weight | Tracking | Tailwind | Use |
| --- | --- | --- | --- | --- | --- |
| `display` | 32 / 40 | 700 | -0.02em | `text-[32px] leading-10 font-bold tracking-tight` | Marketing, empty-state hero |
| `h1` | 24 / 32 | 700 | -0.025em | `text-2xl font-bold tracking-tight` | Page title |
| `h2` | 18 / 28 | 600 | -0.01em | `text-lg font-semibold` | Section title |
| `h3` | 16 / 24 | 600 | 0 | `text-base font-semibold` | Card title |
| `body` | 14 / 22 | 400 | 0 | `text-sm leading-relaxed` | Default body |
| `body-lg` | 16 / 26 | 400 | 0 | `text-base leading-relaxed` | Brief prose, chat |
| `meta` | 12 / 18 | 400 | 0 | `text-xs` | Timestamps, helper text |
| `micro` | 11 / 16 | 500 | 0.06em | `text-[11px] font-medium uppercase tracking-wider` | Table headers, overline |
| `numeric` | inherit | 500 | 0 | `tabular-nums` | Money, counts, SLA |

Body copy colour is `--fg-muted`, not pure white. Pure `--fg` is reserved for
headings and primary values.

---

## 3. Colour Tokens

Tokens are CSS custom properties on `:root` (dark) and `[data-theme="light"]`.
Tailwind v4 maps them through `@theme`. **Components reference tokens, never
hex.** A hex in a `.vue` file is a review failure.

### 3.1 Dark (default)

| Token | Value | Notes |
| --- | --- | --- |
| `--bg-base` | `#0B0F19` | App canvas |
| `--bg-surface` | `#111827` | Cards, panels |
| `--bg-elevated` | `#1F2937` | Modals, dropdowns, popovers |
| `--bg-inset` | `#0D121D` | Table zebra, code blocks |
| `--border` | `#374151` | 1px hairlines |
| `--border-strong` | `#4B5563` | Focused inputs, selected rows |
| `--fg` | `#F9FAFB` | Headings, primary values |
| `--fg-muted` | `#9CA3AF` | Body text (7.0:1 on surface) |
| `--fg-subtle` | `#6B7280` | Disabled, placeholders (3.4:1 — never for body) |
| `--primary` | `#6366F1` | Brand accent, active nav |
| `--primary-hover` | `#4F46E5` | Hover |
| `--primary-fg` | `#FFFFFF` | Text on primary |
| `--primary-soft` | `rgba(99,102,241,0.14)` | Selected row, active chip |
| `--success` | `#10B981` | Approved, paid |
| `--info` | `#3B82F6` | In progress |
| `--warning` | `#F59E0B` | Pending review, SLA at risk |
| `--danger` | `#EF4444` | Revision, destructive |
| `--focus-ring` | `#818CF8` | 2px offset ring |

### 3.2 Light

| Token | Value |
| --- | --- |
| `--bg-base` | `#F8FAFC` |
| `--bg-surface` | `#FFFFFF` |
| `--bg-elevated` | `#FFFFFF` |
| `--bg-inset` | `#F1F5F9` |
| `--border` | `#E2E8F0` |
| `--border-strong` | `#CBD5E1` |
| `--fg` | `#0F172A` |
| `--fg-muted` | `#475569` |
| `--fg-subtle` | `#64748B` |
| `--primary` | `#4F46E5` |
| `--primary-hover` | `#4338CA` |
| `--primary-fg` | `#FFFFFF` |
| `--primary-soft` | `rgba(79,70,229,0.10)` |
| `--success` | `#059669` |
| `--info` | `#2563EB` |
| `--warning` | `#D97706` |
| `--danger` | `#DC2626` |
| `--focus-ring` | `#6366F1` |

### 3.3 Contrast contract (WCAG AA, 4.5:1 for body text)

| Pair | Ratio | Verdict |
| --- | --- | --- |
| `--fg` on `--bg-base` | 17.8:1 | pass |
| `--fg-muted` on `--bg-surface` | 7.0:1 | pass |
| `--fg-subtle` on `--bg-surface` | 3.4:1 | **fail** — disabled/placeholder only |
| white on `--primary` `#6366F1` (dark) | 4.47:1 | pass at ≥18.66px bold / 24px regular; **fail** for 14px labels |
| white on `--primary-hover` `#4F46E5` | 6.3:1 | pass |
| white on `--primary` `#4F46E5` (light) | 6.3:1 | pass |
| `#0B0F19` on `--warning` `#F59E0B` | 10.9:1 | pass |
| white on `--danger` `#EF4444` | 3.8:1 | **fail** — use `#DC2626` for text-bearing fills |

**Rule:** filled buttons using `--primary` at 14px must use `--primary-hover`
as the fill (`#4F46E5`), or render white text at 18.66px+ bold. Badges always
use the *soft* style (tinted background + bright foreground), never solid fills
with white text.

### 3.4 Status token mapping

Canonical mapping. See `UI_SCREEN_SPECS.md` §2 for the column layout.

| DB status | Label | Tone token | Soft bg | Foreground |
| --- | --- | --- | --- | --- |
| `pending_brief` | Brief Pending | `warning` | `rgba(245,158,11,0.14)` | `#FBBF24` |
| `in_queue` | Queue | `neutral` | `rgba(148,163,184,0.14)` | `#CBD5E1` |
| `in_progress` | In Progress | `info` | `rgba(59,130,246,0.14)` | `#93C5FD` |
| `under_review` | Client Review | `warning` | `rgba(245,158,11,0.14)` | `#FBBF24` |
| `revision_requested` | Revision | `danger` | `rgba(239,68,68,0.14)` | `#FCA5A5` |
| `completed` | Completed | `success` | `rgba(16,185,129,0.14)` | `#6EE7B7` |
| `cancelled` | Cancelled | `neutral` | `rgba(100,116,139,0.14)` | `#94A3B8` |
| `paid` | Paid | `success` | as above | `#6EE7B7` |
| `pending` | Pending | `warning` | as above | `#FBBF24` |
| `failed` / `refunded` | Failed / Refunded | `danger` / `neutral` | as above | — |

`approved` exists as a DB enum value but is **not** a UI status. Approval is
expressed by `completed` + `is_approved = true` on the deliverable.

---

## 4. Spacing, Radius, Elevation

**Spacing** — 4px grid. Allowed steps only:

| Token | px | Tailwind |
| --- | ---: | --- |
| `space-1` | 4 | `p-1` |
| `space-2` | 8 | `p-2` |
| `space-3` | 12 | `p-3` |
| `space-4` | 16 | `p-4` |
| `space-5` | 20 | `p-5` |
| `space-6` | 24 | `p-6` |
| `space-8` | 32 | `p-8` |
| `space-10` | 40 | `p-10` |
| `space-12` | 48 | `p-12` |
| `space-16` | 64 | `p-16` |

Card padding `space-4` (compact) / `space-6` (default). Section gap `space-6`.
Page gutter `space-4` mobile / `space-8` desktop. `space-7`, `space-9`,
`space-11` are **not** used — they break rhythm.

**Radius**

| Token | Value | Use |
| --- | --- | --- |
| `--radius-sm` | 6px | Chips, badges, small inputs |
| `--radius-md` | 8px (`rounded-lg`) | Buttons, inputs, table cells |
| `--radius-lg` | 12px (`rounded-xl`) | Cards, panels |
| `--radius-xl` | 16px (`rounded-2xl`) | Modals, full-screen sheets |
| `--radius-full` | 9999px | Avatars, pills |

**Elevation**

| Token | Value | Use |
| --- | --- | --- |
| `--shadow-sm` | `0 1px 2px rgb(0 0 0 / .24)` | Cards |
| `--shadow-md` | `0 8px 24px rgb(0 0 0 / .32)` | Dropdowns |
| `--shadow-lg` | `0 24px 48px rgb(0 0 0 / .48)` | Modals |

Light mode uses the same offsets with `rgb(15 23 42 / .08/.12/.18)`.

**Z-index**

| Layer | Value |
| --- | ---: |
| base | 0 |
| sticky header | 30 |
| dropdown / popover | 40 |
| drawer / sheet | 50 |
| modal | 60 |
| toast | 70 |

---

## 5. Motion

| Token | Value |
| --- | --- |
| `--dur-fast` | 120ms |
| `--dur-base` | 180ms |
| `--dur-slow` | 280ms |
| `--ease-out` | `cubic-bezier(0.16, 1, 0.3, 1)` |
| `--ease-in-out` | `cubic-bezier(0.4, 0, 0.2, 1)` |

Rules:

- Animate `transform` and `opacity` only. Never `height`/`width` on lists.
- Respect `@media (prefers-reduced-motion: reduce)` — all durations to `1ms`,
  no confetti, no parallax.
- The approve confetti runs **once**, ≤ 1.2s, and is skipped entirely under
  reduced motion.
- No skeleton shimmer under reduced motion; use a static placeholder block.

---

## 6. Component Contracts

Each component is defined by props, events, and required states. A component
shipped without its empty/loading/error state is incomplete.

### `<AppButton>`

```ts
interface AppButtonProps {
  variant?: 'primary' | 'secondary' | 'outline' | 'ghost' | 'danger'; // default 'primary'
  size?: 'sm' | 'md' | 'lg';            // 32 / 40 / 48 px height
  type?: 'button' | 'submit';           // default 'button'
  loading?: boolean;                    // spinner + aria-busy, blocks click
  disabled?: boolean;
  block?: boolean;                      // w-full
  iconLeft?: Component;
  iconRight?: Component;
}
// emits: click (suppressed while loading/disabled)
```

- Primary fill uses `--primary-hover` when label is 14px (see §3.3).
- Loading replaces the label with a spinner **and keeps the width** (min-width
  from the label) so the layout does not jump.
- `disabled` uses `opacity: .5` + `cursor: not-allowed`; no hover state.

### `<StatusBadge>`

```ts
interface StatusBadgeProps {
  status: ServiceStatus | OrderStatus;
  size?: 'sm' | 'md';  // 20 / 24 px height
  dot?: boolean;       // default true
}
```

Soft fill only (§3.4). Never accepts a raw colour prop — mapping is centralised.

### `<DataTable>`

```ts
interface DataTableProps<Row> {
  columns: Array<{ key: string; label: string; sortable?: boolean; align?: 'left'|'right'|'center'; width?: string }>;
  rows: Row[];
  rowKey: (row: Row) => string;
  loading?: boolean;      // skeleton rows, keeps header
  error?: string | null;  // inline retry panel
  emptyTitle?: string;
  emptyDescription?: string;
  emptyAction?: { label: string; to?: string; onClick?: () => void };
  stickyHeader?: boolean;
  page?: number;
  perPage?: number;
  total?: number;
  selectable?: boolean;
}
// emits: sort({key, dir}), page-change(page), row-click(row), selection-change(rows)
```

- Sticky header at `z-30`, `--bg-surface` with a 1px bottom border.
- Numeric columns right-aligned with `tabular-nums`.
- Empty state replaces the body, not the header.
- Below 768px the table becomes a card list; never horizontal scroll for
  primary data.

### `<ModalSheet>`

```ts
interface ModalSheetProps {
  open: boolean;
  variant?: 'center' | 'drawer-right' | 'sheet-bottom'; // default 'center'
  title: string;
  description?: string;
  size?: 'sm' | 'md' | 'lg';   // 384 / 512 / 720 px
  dismissible?: boolean;        // default true
  busy?: boolean;               // blocks close during submit
}
// emits: close
```

- Focus trap, `Esc` closes unless `busy`, focus returns to the trigger.
- On mobile, `center` degrades to `sheet-bottom` at full height.
- Destructive confirmations require an explicit verb label ("Cancel order"),
  never "OK".

### `<FileUploadDropzone>`

```ts
interface FileUploadDropzoneProps {
  accept: string;          // 'image/png,image/webp,application/pdf'
  maxSizeMb: number;
  multiple?: boolean;
  disabled?: boolean;
}
// emits: files(File[]), progress({file, percent}), error({file, reason})
```

- Validates type and size client-side, then re-validates server-side.
- Shows filename, size, per-file progress bar, and a remove action.
- Rejects with a reason string, not a red border alone.
- Drag-over state uses `--primary-soft` + dashed `--primary` border.

### `<MoneyText>`

```ts
interface MoneyTextProps {
  amount: number | string;     // Decimal serialized as string
  currency?: string;           // ISO 4217, default from workspace
  locale?: string;             // default from user profile
  size?: 'sm' | 'md' | 'lg';
  muted?: boolean;
}
```

- Formatting via `Intl.NumberFormat(locale, { style: 'currency', currency })`.
- **USD is the default** workspace currency. BDT renders as `৳` (U+09F3) from
  the Bengali subset; no fallback font.
- Never do arithmetic on the formatted string. Amounts arrive as decimal
  strings and are displayed verbatim through `Intl`.
- Negative amounts (refunds) render with `--success` and an explicit `+`.

### `<EmptyState>`, `<SlaTimer>`, `<KanbanCard>`, `<Toast>`

- `EmptyState`: icon, title, one-sentence description, single primary action.
- `SlaTimer`: `due_at` → live countdown. `>24h` neutral, `≤24h` warning,
  overdue danger + "Overdue by …" text. `aria-live="off"` (announcing every
  second is hostile); announce only on threshold crossing.
- `KanbanCard`: order number (`numeric`), client, service, SLA, assignee
  avatar. Draggable; keyboard alternative is a "Move to…" menu.
- `Toast`: max 3 stacked, 5s auto-dismiss (errors 8s, never auto-dismiss for
  `revision_limit_reached`), `aria-live="polite"`, dismiss button.

---

## 7. Accessibility Baseline

- Visible focus ring on every interactive element: `outline: 2px solid
  var(--focus-ring); outline-offset: 2px`. Never `outline: none` without a
  replacement.
- Minimum touch target 44×44px, including icon buttons.
- Every icon-only button has `aria-label`.
- Form errors are linked with `aria-describedby` and announced via
  `role="alert"`.
- Kanban drag-and-drop must work with keyboard (menu fallback) and expose
  `aria-grabbed`/`aria-dropeffect` or an equivalent live-region message.
- Do not encode status by colour alone — the badge always has a label.
- Language: `<html lang="bn">` for Bengali-first screens; wrap mixed English
  strings in `<span lang="en">` when the screen is Bengali.

---

## 8. Do / Don't

| Do | Don't |
| --- | --- |
| Reference `var(--token)` | Hardcode `#6366F1` in a component |
| Use `StatusBadge` for status | Write a per-screen colour ternary |
| Show `allowed_actions` from the API | Recompute state rules in the client |
| Reserve space for loading content | Let the layout jump on load |
| Use the self-hosted Anek Bangla | Import from Google Fonts or a CDN |
| Keep body line-height ≥ 1.6 | Set `leading-none` on Bengali text |
