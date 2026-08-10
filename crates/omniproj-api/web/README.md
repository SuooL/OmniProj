# OmniProj cockpit (web UI)

The dashboard SPA: **React + Vite + Tailwind v4 + TanStack Query**. It renders the
local `omniproj-api` endpoints into the situational cockpit (charter §8).

## Build contract

The built `dist/` is **committed** and embedded into the `omniproj` binary via `rust-embed`
(`crates/omniproj-api/src/lib.rs`). This is deliberate: `cargo install --git` users have no
Node toolchain, so the shipped binary must already contain the UI.

**Whenever you change anything under `web/`, rebuild and commit `dist/`:**

```sh
cd crates/omniproj-api/web
npm install      # first time (writes package-lock.json)
npm run build    # → dist/  (commit it)
```

CI (`.github/workflows/ci.yml` → `web-dist-fresh`) rebuilds `dist/` and fails the PR if
it differs from what's checked in, so a forgotten rebuild can't merge.

## Dev

```sh
omniproj dashboard --port 7700      # serve the API (+ the embedded prod UI)
cd crates/omniproj-api/web && npm run dev   # Vite dev server, proxies /api → :7700
```

## Charter guardrails baked into the UI

- **Pull, not push** (§8): no refetch-on-focus / interval; refresh is a button.
- **Facts, not scores** (§5 原则3, §8 护栏 ii): cards show counts/activity, never a
  synthesized "health" number or priority ranking. Sort order (recent activity) is a
  neutral fact, labeled as such.
- **Thresholds visible** (§8 护栏 i): the activity-dot legend states its day cutoffs.
