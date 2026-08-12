# legacy-src — pre-R0 UI archive

These are the milestone-era cockpit components (Attend / Record / Advance, decisions, Git
graph, settings, reminders) plus the pre-R0 `api.ts` they depend on. They are kept verbatim
as a self-contained archive so the later R1/R2/R3 redesign can lift proven pieces.

This directory is **outside** `tsconfig.json`'s `src` include and is imported by nothing in
`src/`, so it is neither type-checked nor bundled into the shipped app — exactly like the
Rust `crates/omniproj-desktop/src/legacy.rs` archive. Do not import from here in `src/`.
