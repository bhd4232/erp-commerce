# Anek Bangla (self-hosted)

Bengali-first UI typeface used across the whole product. The font files live in
this repository and are served by the application itself — **no CDN, no
third-party request at runtime.**

## Provenance

| Field | Value |
| --- | --- |
| Family | Anek Bangla |
| Upstream | https://github.com/EkType/Anek |
| Package | `@fontsource-variable/anek-bangla@5.3.0` |
| Download host | `cdn.jsdelivr.net` (build-time only) |
| Axes | `wght` 100–800 (variable, one file per subset) |
| License | SIL Open Font License 1.1 — see [`OFL.txt`](./OFL.txt) |

## Files

| File | Subset | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `anek-bangla-bengali-wght-normal.woff2` | Bengali (U+0980–09FE) | 155,676 | `418c763509d64cf632bc7ff30b3d97c1826299a3967d19d8e8fa297343f33ec7` |
| `anek-bangla-latin-wght-normal.woff2` | Latin | 42,816 | `9d14431a2774254bf494d4bfbd523017d8141f2d283ad67758058906700e840a` |
| `anek-bangla-latin-ext-wght-normal.woff2` | Latin-ext | 2,832 | `4c123e2cfedc4bb6ab03408467f4730e0790177a02fe2f711d89cf398f6eae59` |
| `OFL.txt` | — | 4,375 | `bbb7ff2609b44e3784df302414e2e6bda6a1ecc46cf800a3ac0d933b7840a234` |

Total payload: **201 KB**. Only the Bengali file is preloaded; the other two are
small and load on demand.

## Usage

Import once, before any other stylesheet:

```css
@import "./assets/fonts/anek-bangla/fonts.css";
```

Then `var(--font-sans)` and `var(--font-bangla)` resolve to the self-hosted
family. Tailwind v4 theme mapping:

```css
@theme {
  --font-sans: "Anek Bangla Variable", "Inter", system-ui, sans-serif;
  --font-bangla: "Anek Bangla Variable", "Noto Sans Bengali", sans-serif;
}
```

Preload the Bengali subset in `index.html`:

```html
<link
  rel="preload"
  href="/fonts/anek-bangla/anek-bangla-bengali-wght-normal.woff2"
  as="font"
  type="font/woff2"
  crossorigin
/>
```

> The `crossorigin` attribute is mandatory even for same-origin fonts. Without
> it the browser fetches the file twice.

### Note on the Taka sign

`৳` is `U+09F3`, which is inside the Bengali `unicode-range`. Currency output
never falls back to a system font.

### Note on line height

Bengali matras sit below the Latin baseline. Keep body copy at
`line-height ≥ 1.6`; `--leading-bangla: 1.75` is provided for Bengali blocks.
Do not use `line-height: 1` for Bengali headings — the *hasanta* and *ukar*
get clipped.

## Updating the font

1. Check the new version on https://www.npmjs.com/package/@fontsource-variable/anek-bangla
2. Download the three `wght` files plus `LICENSE` from
   `https://cdn.jsdelivr.net/npm/@fontsource-variable/anek-bangla@<version>/`
3. Replace the files and update the byte counts and hashes in this table.
4. Re-run a visual check of Bengali numerals (`০১২৩৪৫৬৭৮৯`) and conjuncts
   (`ক্ষ`, `জ্ঞ`, `ন্ত্র`).

## Why not the system font?

Windows ships no Bengali font by default, macOS coverage varies, and Linux
distributions disagree. Self-hosting guarantees identical metrics and shaping
on Web, Tauri Desktop, and Tauri Mobile.
