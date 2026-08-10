//! The wire vocabulary. Adding fields is backward-compatible (serde defaults), which
//! is the "versionable" property the spec wanted from gRPC.

use serde::{Deserialize, Serialize};

/// A CLI → daemon request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Request {
    /// Liveness probe (used by the CLI's lazy-start).
    Ping,
    /// Full daemon + per-project status snapshot.
    Status,
    /// Ask the daemon to re-read the project registry and watch newly-added projects.
    /// Used by `omniproj add/remove`; best-effort when no daemon is running.
    Reload,
}

/// A daemon → CLI response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Response {
    Pong { pid: u32 },
    Ack,
    Status(StatusResponse),
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub pid: u32,
    /// RFC3339 daemon start time.
    pub started_at: String,
    /// Name of the project currently being distilled, if any.
    pub in_flight: Option<String>,
    pub projects: Vec<ProjectStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectStatus {
    pub name: String,
    pub hash: String,
    pub path: String,
    /// Whether the daemon currently has an fs watch on this project's worktree.
    pub watched: bool,
    /// Human-readable last-distill marker (e.g. `distilled 2026-06-06T…` or `never`).
    pub last_activity: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_response_round_trips() {
        let resp = Response::Status(StatusResponse {
            pid: 4242,
            started_at: "2026-06-07T00:00:00Z".into(),
            in_flight: Some("proj".into()),
            projects: vec![ProjectStatus {
                name: "proj".into(),
                hash: "deadbeef".into(),
                path: "/p".into(),
                watched: true,
                last_activity: "never".into(),
            }],
        });
        let json = serde_json::to_vec(&resp).unwrap();
        let back: Response = serde_json::from_slice(&json).unwrap();
        match back {
            Response::Status(s) => {
                assert_eq!(s.pid, 4242);
                assert_eq!(s.in_flight.as_deref(), Some("proj"));
                assert_eq!(s.projects.len(), 1);
                assert!(s.projects[0].watched);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn request_variants_round_trip() {
        for req in [Request::Ping, Request::Status, Request::Reload] {
            let json = serde_json::to_vec(&req).unwrap();
            let _back: Request = serde_json::from_slice(&json).unwrap();
        }
    }

    #[test]
    fn ack_round_trips() {
        let json = serde_json::to_vec(&Response::Ack).unwrap();
        let back: Response = serde_json::from_slice(&json).unwrap();
        assert!(matches!(back, Response::Ack));
    }
}
