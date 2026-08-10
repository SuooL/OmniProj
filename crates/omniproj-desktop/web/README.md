# OmniProj cockpit (desktop UI)

The desktop app's webview frontend: **React + Vite + Tailwind v4 + TanStack Query**. It
renders the Attend-layer overview (charter §8) and talks to the Rust backend over **Tauri
IPC** — `src/api.ts` calls `invoke("get_projects")`, handled in
`crates/omniproj-desktop/src/main.rs`. (Pre-pivot this SPA was served by the now-removed
`omniproj-api` axum server; it now lives inside the desktop crate.)

## Build

Tauri drives the frontend build via `tauri.conf.json` (`beforeDevCommand` /
`beforeBuildCommand` → `npm --prefix web run …`, `frontendDist: web/dist`):

```sh
cd crates/omniproj-desktop/web
npm install      # first time (writes package-lock.json)
npm run dev      # Vite dev server on :5173 (Tauri devUrl)
npm run build    # → dist/  (what Tauri bundles)
```

Run the app from the crate root with `cargo tauri dev` (or `cargo run -p omniproj-desktop`
once a dist exists).

## Charter guardrails baked into the UI

- **Pull, not push** (§8): no refetch-on-focus / interval; refresh is a button.
- **Facts, not scores** (§5 原则3, §8 护栏 ii): cards show counts/activity, never a
  synthesized "health" number or priority ranking. Sort order (recent activity) is a
  neutral fact, labeled as such.
- **Thresholds visible** (§8 护栏 i): the activity-dot legend states its day cutoffs.
