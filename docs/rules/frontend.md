# Rule: Frontend (Vue 3 + TypeScript)

**Tier:** 3 mostly; 2 for the API/state guards that can be tested.

- Vue 3.5+, `<script setup lang="ts">`, Composition API only.
- Vite + Tailwind v4 CSS-first `@theme`; no `tailwind.config.js`.
- All HTTP goes through `src/api/client.ts`; views never call `fetch`.
- Never hardcode a hex colour; use the CSS custom properties from
  `docs/UI_DESIGN_SYSTEM.md`.
- Never recompute state rules. Render buttons from the API's
  `allowed_actions`; do not check `status === ...` in a component.
- Money renders through `<MoneyText>`; never arithmetic on formatted strings;
  use `tabular-nums`.
- Font: self-hosted `Anek Bangla Variable`; never a CDN.
- Bengali text follows `docs/I18N.md`: `line-height ≥ 1.6`, `lang` set, no
  `leading-none` headings.
- Every screen ships loading, empty, error and disabled states.
- Signed URLs are never stored; open them immediately and discard.
- The frontend is currently a spec, not a scaffolded app. Do not claim a screen
  works until it is built and viewed.
