//! omniproj-daemon — orchestration (spec §5 / §7 "Daemon/Orchestrator").
//!
//! This crate glues capture → distill → verify → store into one idempotent
//! [`refresh_project`] step, gated by the deterministic staleness floor (spec §5):
//! distill **only when the change fingerprint moved**, stay silent otherwise.
//!
//! The background watcher + 24h floor timer + IPC server are built on top of this
//! in later slices; for now both the CLI (`omniproj briefing` / `omniproj refresh`) and a
//! future daemon loop call the same orchestration so behavior can't drift.

pub mod daemon;
pub mod opinion;
pub mod refresh;

pub use daemon::{run, DaemonOpts};
pub use opinion::{generate_opinion, OpinionOpts, OpinionOutput};
pub use refresh::{distill_and_write, refresh_project, Distilled, RefreshOpts, RefreshOutcome};
