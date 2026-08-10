//! omniproj-ipc — daemon ↔ CLI transport (spec §7 "daemon↔CLI IPC").
//!
//! The spec's first choice was tonic gRPC over a Unix socket (ref atuin). For v1 we
//! ship the same surface — a typed, versionable request/response over
//! `~/.omniproj/daemon.sock` — as **length-delimited-by-half-close JSON**, which needs
//! no `protoc`/codegen and almost no deps. The crate boundary is unchanged, so a
//! later move to gRPC (e.g. once streamed distill progress is wanted) is localized to
//! this crate. See the v1 spec §7 note on this deviation.
//!
//! Protocol (one request/response per connection):
//! 1. client connects, writes a JSON [`Request`], then half-closes its write half;
//! 2. server reads to EOF, processes, writes a JSON [`Response`], closes.

pub mod client;
pub mod proto;
pub mod server;

pub use proto::{ProjectStatus, Request, Response, StatusResponse};

use std::path::PathBuf;

/// The daemon's Unix socket: `~/.omniproj/daemon.sock`.
pub fn socket_path() -> PathBuf {
    omniproj_core::omniproj_home().join("daemon.sock")
}
