# Tauri v2 Desktop & Mobile Integration Specification

The web SPA runs unchanged inside the Tauri shell. Native code is a thin
adapter: no business rules, no SQL, no direct storage credentials.

---

## 1. Targets

| Target | Bundle id | Notes |
| --- | --- | --- |
| Windows 10/11 | `com.bhd4232.erp` | MSI + NSIS |
| macOS 12+ | `com.bhd4232.erp` | DMG, universal binary |
| Linux | `com.bhd4232.erp` | AppImage + deb |
| Android 8+ | `com.bhd4232.erp` | AAB |
| iOS 15+ | `com.bhd4232.erp` | IPA (later phase) |

Deep-link scheme: **`erp://`** (matches `UI_USER_FLOWS.md`). The earlier
`agencyerp://` proposal is rejected to avoid two schemes in one codebase.

```
erp://orders/{id}            → /services/orders/{id}
erp://orders/{id}/brief      → /services/orders/{id}/brief
erp://digital/vault          → /digital/vault
```

---

## 2. Required Capabilities

Tauri v2 uses explicit permissions in `src-tauri/capabilities/default.json`.
Only these are granted:

| Capability | Permission | Used for |
| --- | --- | --- |
| HTTP | `http:default` scoped to the API origin + storage host | downloads, API |
| Dialog | `dialog:allow-save`, `dialog:allow-open` | save-as for deliverables |
| FS | `fs:allow-write-file` scoped to `$DOWNLOAD/**` and `$DOCUMENT/**` | writing files |
| Notification | `notification:default` | order status alerts |
| Shell open | `opener:allow-open-path` | reveal downloaded file |
| Deep link | `deep-link:default` | `erp://` routing |
| Updater | `updater:default` | signed auto-update |
| Store | `store:default` | local prefs (theme, last route) |
| Single instance | `single-instance:default` | focus existing window |

Nothing else. No `shell:allow-execute`, no arbitrary path access.

---

## 3. Native File Download

Web builds cannot choose a folder; the desktop build can.

```
User clicks "Download"
  └─ POST /digital/download/{id}            (unchanged API)
      └─ 200 { download_url, filename, expires_at }
          ├─ Web:     window.open(url)
          └─ Tauri:   plugin-http streams GET url
                        └─ dialog.save() → user picks folder (default: Downloads)
                            └─ fs write in 1 MiB chunks
                                └─ progress event to the UI
                                    └─ notification "Saved to …" + "Open folder"
```

Rules:

- The signed URL is fetched by the native layer, never embedded in a webview
  `<a download>`.
- Cancel deletes the partial file.
- Verify `Content-Length` when present; mismatch → error, no file.
- Filenames are sanitized before writing (no separators, no `..`).
- The URL is never persisted to the store or the filesystem.

---

## 4. Notifications

| Event | Channel |
| --- | --- |
| Brief submitted (staff) | system notification + tray badge |
| Revision requested (staff) | system notification, high priority |
| Deliverable ready (client) | system notification |
| Order completed / payment received | silent tray update |

- Desktop: system tray icon with unread count; clicking focuses the window.
- Mobile: local notification; push (FCM/APNs) is a later phase — v1 polls
  `GET /notifications` on resume.
- Never include order contents or client PII in the notification body; use
  "New revision request · SVC-20260101-0007".

---

## 5. Offline Behaviour

Read-only cache, never offline writes.

- Cache: `auth/me`, last `my-vault`, last orders page, theme, last route.
- Storage: `tauri-plugin-store` (JSON), capped at 2 MB, pruned after 7 days.
- Offline: persistent banner, cached screens marked "cached · <time>", all
  mutations disabled with a tooltip.
- Reconnect: refetch active view, replay nothing (no write queue).
- Conflict: server always wins; the UI refetches rather than merging.

---

## 6. Window & Layout

| Concern | Rule |
| --- | --- |
| Min size | 1024×720 desktop |
| Title bar | native decorations off; custom top bar with drag region |
| Sidebar | same breakpoints as web (`<768px` → bottom nav) |
| Preview player | full-screen sheet on mobile |
| Deep link while running | focus window, then route |

---

## 7. Security

- JWT stored in the OS keychain (`tauri-plugin-stronghold` or keyring plugin),
  never in `localStorage` on desktop.
- API origin allow-listed in `tauri.conf.json`; the webview cannot call any
  other host.
- CSP: `default-src 'self'; connect-src 'self' https://api.<domain>; img-src
  'self' data: blob: https://<storage-host>; style-src 'self' 'unsafe-inline'`.
- Auto-update must be signed; the public key is pinned in `tauri.conf.json`.
- No hidden webview, no remote code loading.

---

## 8. Build Pipeline

```bash
cd frontend
npm ci
npm run tauri build              # current platform
npm run tauri build -- --target aarch64-apple-darwin
npm run tauri android build      # AAB
```

CI builds per-OS on native runners; artifacts are signed. Mobile signing keys
live in CI secrets, never in the repo.

---

## 9. Acceptance Checklist

- [ ] `erp://orders/{id}` opens the correct order on a cold start
- [ ] Download saves to a user-chosen folder with a progress indicator
- [ ] Cancel leaves no partial file
- [ ] Notification opens the related order
- [ ] Offline banner appears within 2 s of losing the network
- [ ] Cached screens are visually marked
- [ ] No token in `localStorage` on desktop
- [ ] Capability file lists only the permissions in §2
