//! omniproj-api — the local web dashboard (spec §7 L4, §3 "两个无状态前端").
//!
//! By default this is a read view over `~/.omniproj` (Layer 2/3) plus the daemon's IPC
//! status. The one mutating route is explicit `POST /opinion`, which reuses the
//! same grounded second-opinion orchestration as the CLI and writes a revertable
//! store commit. Binds 127.0.0.1 only — this is a local tool, not a service.
//!
//! The SPA is a React + Vite + Tailwind app under `web/`, built to `web/dist` and
//! embedded into the binary via `rust-embed` (spec §339 sanctioned this swap; the API
//! surface is unchanged). `cargo install` users need no Node — the committed `dist/`
//! is the shipped UI. To change the UI: edit `web/`, `npm run build`, commit `dist/`.

use axum::extract::{Path as AxPath, State};
use axum::http::{header, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};

/// The committed React build (`web/dist`), embedded at compile time so the shipped
/// binary carries the whole cockpit — no Node, no external files (charter §5 原则1
/// portable). Rebuild with `web/` + `npm run build`; CI verifies dist is fresh.
#[derive(RustEmbed)]
#[folder = "web/dist"]
struct WebAssets;

#[derive(Serialize)]
struct ProjectSummary {
    name: String,
    hash: String,
    path: String,
    last_distilled: Option<String>,
    last_head: Option<String>,
}

#[derive(Serialize)]
struct ProjectDetail {
    name: String,
    hash: String,
    path: String,
    last_distilled: Option<String>,
    last_head: Option<String>,
    trust: TrustSummary,
    user_model: UserModelSummary,
    /// State files keyed by kind: briefing / decisions / open / opinion / learned.
    files: serde_json::Map<String, serde_json::Value>,
}

#[derive(Serialize)]
struct TrustSummary {
    present: bool,
    clean: Option<bool>,
    at: Option<String>,
    flagged_hashes: Vec<String>,
    flagged_paths: Vec<String>,
}

#[derive(Serialize)]
struct UserModelSummary {
    active: usize,
    dimensions: Vec<UserModelDimension>,
}

#[derive(Serialize)]
struct UserModelDimension {
    name: String,
    enabled: bool,
    chars: usize,
    over_budget: bool,
}

#[derive(Serialize)]
struct DaemonStatus {
    running: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    in_flight: Option<String>,
}

async fn projects() -> Json<Vec<ProjectSummary>> {
    let list = omniproj_core::list_projects()
        .into_iter()
        .map(|m| ProjectSummary {
            name: m.name,
            hash: m.hash,
            path: m.path,
            last_distilled: m.last_distilled,
            last_head: m.last_head,
        })
        .collect();
    Json(list)
}

async fn project_detail(AxPath(hash): AxPath<String>) -> Result<Json<ProjectDetail>, StatusCode> {
    // The hash arrives from the URL: refuse anything that isn't a bare hex id so it
    // can't traverse paths (defense in depth — project_dir joins it into a path).
    if !hash.chars().all(|c| c.is_ascii_hexdigit()) || hash.len() > 32 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let meta = omniproj_core::load_meta(&hash).ok_or(StatusCode::NOT_FOUND)?;
    let auto = omniproj_core::auto_dir(&hash);
    let mut files = serde_json::Map::new();
    for kind in ["briefing", "decisions", "open", "opinion"] {
        if let Ok(text) = std::fs::read_to_string(auto.join(format!("{kind}.md"))) {
            files.insert(kind.to_string(), serde_json::Value::String(text));
        }
    }
    if let Ok(text) = std::fs::read_to_string(omniproj_core::learned_path(&hash)) {
        files.insert("learned".to_string(), serde_json::Value::String(text));
    }
    Ok(Json(ProjectDetail {
        name: meta.name,
        hash: meta.hash,
        path: meta.path,
        last_distilled: meta.last_distilled,
        last_head: meta.last_head,
        trust: trust_summary(&hash),
        user_model: user_model_summary(),
        files,
    }))
}

fn trust_summary(hash: &str) -> TrustSummary {
    let path = omniproj_core::cache_dir(hash).join("verify-report.json");
    let Ok(text) = std::fs::read_to_string(path) else {
        return TrustSummary {
            present: false,
            clean: None,
            at: None,
            flagged_hashes: Vec::new(),
            flagged_paths: Vec::new(),
        };
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        return TrustSummary {
            present: true,
            clean: None,
            at: None,
            flagged_hashes: Vec::new(),
            flagged_paths: Vec::new(),
        };
    };
    let strings = |key: &str| -> Vec<String> {
        v.get(key)
            .and_then(|x| x.as_array())
            .map(|xs| {
                xs.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };
    TrustSummary {
        present: true,
        clean: v.get("clean").and_then(|x| x.as_bool()),
        at: v.get("at").and_then(|x| x.as_str()).map(str::to_string),
        flagged_hashes: strings("flagged_hashes"),
        flagged_paths: strings("flagged_paths"),
    }
}

fn user_model_summary() -> UserModelSummary {
    let model = omniproj_core::UserModel::load();
    let dimensions = model
        .dimensions
        .iter()
        .map(|d| {
            let chars = d.body.chars().count();
            UserModelDimension {
                name: d.name.clone(),
                enabled: d.enabled,
                chars,
                over_budget: d.enabled && chars > omniproj_core::USER_MODEL_DIM_CAP_CHARS,
            }
        })
        .collect::<Vec<_>>();
    let active = dimensions
        .iter()
        .filter(|d| d.enabled && d.chars > 0)
        .count();
    UserModelSummary { active, dimensions }
}

#[derive(serde::Deserialize)]
struct SearchParams {
    q: String,
    #[serde(default = "default_limit")]
    limit: usize,
}
fn default_limit() -> usize {
    10
}

#[derive(Serialize)]
struct SearchHit {
    source: String,
    role: String,
    mtime: f64,
    snippet: String,
}

#[derive(Deserialize)]
struct OpinionRequest {
    #[serde(default)]
    ignore: Vec<String>,
    model: Option<String>,
}

#[derive(Serialize)]
struct OpinionResponse {
    text: String,
    provider_label: String,
    ignored: Vec<String>,
    flagged_hashes: Vec<String>,
    flagged_paths: Vec<String>,
}

async fn project_search(
    AxPath(hash): AxPath<String>,
    axum::extract::Query(p): axum::extract::Query<SearchParams>,
) -> Result<Json<Vec<SearchHit>>, StatusCode> {
    if !hash.chars().all(|c| c.is_ascii_hexdigit()) || hash.len() > 32 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let meta = omniproj_core::load_meta(&hash).ok_or(StatusCode::NOT_FOUND)?;
    let limit = p.limit.min(50);
    // capture + index in a blocking task — bundled sqlite + fs parsing are sync
    let hits = tokio::task::spawn_blocking(move || {
        omniproj_index::search_project(std::path::Path::new(&meta.path), &p.q, limit)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(
        hits.into_iter()
            .map(|h| SearchHit {
                source: h.source,
                role: h.role,
                mtime: h.mtime,
                snippet: h.snippet,
            })
            .collect(),
    ))
}

async fn project_opinion(
    AxPath(hash): AxPath<String>,
    Json(req): Json<OpinionRequest>,
) -> Result<Json<OpinionResponse>, (StatusCode, String)> {
    if !hash.chars().all(|c| c.is_ascii_hexdigit()) || hash.len() > 32 {
        return Err((StatusCode::BAD_REQUEST, "invalid project hash".into()));
    }
    let meta = omniproj_core::load_meta(&hash)
        .ok_or((StatusCode::NOT_FOUND, "project not found".to_string()))?;
    let out = omniproj_daemon::generate_opinion(
        std::path::Path::new(&meta.path),
        omniproj_daemon::OpinionOpts {
            model: req.model.as_deref(),
            ignore: req.ignore,
        },
        |m| eprintln!("[omniproj-api] {m}"),
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")))?;
    Ok(Json(OpinionResponse {
        text: out.text,
        provider_label: out.provider_label,
        ignored: out.ignored,
        flagged_hashes: out.verify.flagged,
        flagged_paths: out.verify.flagged_paths,
    }))
}

async fn daemon_status() -> Json<DaemonStatus> {
    match omniproj_ipc::client::request(&omniproj_ipc::Request::Status).await {
        Ok(omniproj_ipc::Response::Status(s)) => Json(DaemonStatus {
            running: true,
            pid: Some(s.pid),
            started_at: Some(s.started_at),
            in_flight: s.in_flight,
        }),
        _ => Json(DaemonStatus {
            running: false,
            pid: None,
            started_at: None,
            in_flight: None,
        }),
    }
}

// ------------------------------------------------------------------- portfolio (cockpit P3)

/// One project's neutral situational facts for the Portfolio glance grid (charter §8).
/// Everything here is a fact (counts, timestamps, activity), NOT a score or ranking —
/// the front-end must not synthesize a "health" number from it (charter §5 原则3, §8
/// 护栏 ii). Sorted by most-recent activity, which is itself a neutral fact.
#[derive(Serialize)]
struct PortfolioCard {
    name: String,
    hash: String,
    path: String,
    last_distilled: Option<String>,
    last_head: Option<String>,
    branch: Option<String>,
    /// Uncommitted lines (`git status --porcelain` count) — a fact, not a judgement.
    dirty: usize,
    /// 16-week commit histogram, oldest → newest (the sparkline).
    commit_weeks: Vec<u32>,
    claude_sessions: usize,
    codex_sessions: usize,
    /// Newest captured session mtime (epoch secs) — "when this project last moved".
    last_activity: Option<f64>,
    /// Open next-action count and, of those, how many are still 未成形.
    notes_open: usize,
    notes_unclear: usize,
    /// Verify-gate state (⚠ count) — surfaced, never ranked.
    trust_flagged: usize,
    trust_clean: Option<bool>,
}

const SPARK_WEEKS: usize = 16;

fn epoch_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Compute one card. Runs git + session capture (sync), so callers dispatch it on a
/// blocking thread. Pure per-project — no shared state.
fn portfolio_card(meta: omniproj_core::ProjectMeta, now: i64) -> PortfolioCard {
    let dir = std::path::PathBuf::from(&meta.path);
    let (branch, dirty, weeks, claude_n, codex_n, last_activity) =
        match omniproj_capture::capture(&dir) {
            Ok(sub) => {
                let (branch, dirty) = sub
                    .git
                    .as_ref()
                    .map(|g| {
                        let d = g.status_porcelain.lines().filter(|l| !l.is_empty()).count();
                        (Some(g.branch.clone()), d)
                    })
                    .unwrap_or((None, 0));
                let last = sub
                    .sessions
                    .iter()
                    .map(|s| s.mtime)
                    .fold(None, |acc, m| Some(acc.map_or(m, |a: f64| a.max(m))));
                (
                    branch,
                    dirty,
                    omniproj_capture::git::commit_weeks(&dir, SPARK_WEEKS, now),
                    sub.claude_n,
                    sub.codex_n,
                    last,
                )
            }
            // Capture can fail (e.g. path went missing) — still show the card from meta.
            Err(_) => (None, 0, vec![0; SPARK_WEEKS], 0, 0, None),
        };
    let (open, unclear) = omniproj_core::NextDoc::load(&meta.hash).counts();
    let trust = trust_summary(&meta.hash);
    PortfolioCard {
        name: meta.name,
        hash: meta.hash,
        path: meta.path,
        last_distilled: meta.last_distilled,
        last_head: meta.last_head,
        branch,
        dirty,
        commit_weeks: weeks,
        claude_sessions: claude_n,
        codex_sessions: codex_n,
        last_activity,
        notes_open: open,
        notes_unclear: unclear,
        trust_flagged: trust.flagged_hashes.len() + trust.flagged_paths.len(),
        trust_clean: trust.clean,
    }
}

async fn portfolio() -> Json<Vec<PortfolioCard>> {
    let now = epoch_now();
    // Each card runs git + capture; do them on the blocking pool, concurrently.
    let metas = omniproj_core::list_projects();
    let handles: Vec<_> = metas
        .into_iter()
        .map(|m| tokio::task::spawn_blocking(move || portfolio_card(m, now)))
        .collect();
    let mut cards = Vec::new();
    for h in handles {
        if let Ok(card) = h.await {
            cards.push(card);
        }
    }
    // Sort by most-recent activity (session mtime, else last_distilled proxy). A neutral
    // fact, explicitly NOT a priority ranking (charter §5 原则3). Newest first.
    cards.sort_by(|a, b| {
        b.last_activity
            .partial_cmp(&a.last_activity)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.name.cmp(&b.name))
    });
    Json(cards)
}

/// Cross-project next-action item for the aggregate notes view (charter §8 explore).
#[derive(Serialize)]
struct NoteRow {
    project: String,
    hash: String,
    id: Option<String>,
    text: String,
    unclear: bool,
}

async fn all_notes() -> Json<Vec<serde_json::Value>> {
    let mut out = Vec::new();
    for m in omniproj_core::list_projects() {
        let doc = omniproj_core::NextDoc::load(&m.hash);
        let items: Vec<NoteRow> = doc
            .items()
            .filter(|t| !t.done)
            .map(|t| NoteRow {
                project: m.name.clone(),
                hash: m.hash.clone(),
                id: t.id.clone(),
                text: t.text.clone(),
                unclear: t.unclear,
            })
            .collect();
        if !items.is_empty() {
            let touched = std::fs::metadata(omniproj_core::next_path(&m.hash))
                .and_then(|md| md.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs());
            out.push(serde_json::json!({
                "project": m.name,
                "hash": m.hash,
                "touched": touched,
                "items": items,
            }));
        }
    }
    // Most-recently-edited list first (neutral fact, not a ranking).
    out.sort_by(|a, b| {
        b.get("touched")
            .and_then(|v| v.as_u64())
            .cmp(&a.get("touched").and_then(|v| v.as_u64()))
    });
    Json(out)
}

/// Serve an embedded static asset by path, guessing content-type from the extension.
/// Unknown paths fall back to `index.html` so the SPA owns client-side routing.
fn serve_asset(path: &str) -> Response {
    let path = path.trim_start_matches('/');
    let file = WebAssets::get(path).or_else(|| WebAssets::get("index.html"));
    match file {
        Some(f) => {
            let mime = content_type(if WebAssets::get(path).is_some() {
                path
            } else {
                "index.html"
            });
            ([(header::CONTENT_TYPE, mime)], f.data).into_response()
        }
        // dist/ missing (a dev binary built before `npm run build`) — say so plainly.
        None => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "web build missing: run `npm run build` in crates/omniproj-api/web\n",
        )
            .into_response(),
    }
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("woff2") => "font/woff2",
        Some("png") => "image/png",
        _ => "application/octet-stream",
    }
}

async fn index() -> Response {
    serve_asset("index.html")
}

async fn asset(AxPath(path): AxPath<String>) -> Response {
    // The `/assets/{*path}` wildcard captures only the sub-path; the embedded file
    // lives under `assets/` in dist.
    serve_asset(&format!("assets/{path}"))
}

/// The set of `Host` header values a loopback dashboard on `port` legitimately
/// answers to. Anything else is a cross-origin request that resolved our port —
/// the DNS-rebinding vector — and is refused before it can read state or write
/// (opinion / notes / clarify) on the user's behalf.
fn allowed_hosts(port: u16) -> [String; 3] {
    [
        format!("127.0.0.1:{port}"),
        format!("localhost:{port}"),
        format!("[::1]:{port}"),
    ]
}

/// DNS-rebinding guard: a browser page served from an attacker domain that
/// resolves to 127.0.0.1 still sends its own `Host` (e.g. `evil.example`),
/// which loopback CORS does not stop. Pinning `Host` to the loopback names
/// closes that gap. A missing `Host` (raw socket / HTTP/1.0) is refused too.
async fn require_local_host(
    State(port): State<u16>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let allowed = allowed_hosts(port);
    let ok = req
        .headers()
        .get(axum::http::header::HOST)
        .and_then(|h| h.to_str().ok())
        .map(|h| allowed.iter().any(|a| a == h))
        .unwrap_or(false);
    if ok {
        next.run(req).await
    } else {
        (
            StatusCode::FORBIDDEN,
            "refused: request Host is not a loopback address (DNS-rebinding guard)\n",
        )
            .into_response()
    }
}

pub fn router(port: u16) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/api/portfolio", get(portfolio))
        .route("/api/notes", get(all_notes))
        .route("/api/projects", get(projects))
        .route("/api/projects/{hash}", get(project_detail))
        .route("/api/projects/{hash}/search", get(project_search))
        .route("/api/projects/{hash}/opinion", post(project_opinion))
        .route("/api/daemon", get(daemon_status))
        // Embedded React assets (index-*.js / index-*.css / …). API routes are matched
        // first; anything else resolves against the embedded dist, else index.html.
        .route("/assets/{*path}", get(asset))
        .layer(axum::middleware::from_fn_with_state(
            port,
            require_local_host,
        ))
}

/// Serve the dashboard on `127.0.0.1:port` until the process is stopped.
pub async fn serve(port: u16) -> anyhow::Result<()> {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    eprintln!("[omniproj] dashboard at http://{addr}/ (Ctrl-C to stop)");
    axum::serve(listener, router(port)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request as HttpRequest;
    use tower::ServiceExt as _; // oneshot

    fn req_with_host(host: Option<&str>) -> HttpRequest<Body> {
        let mut b = HttpRequest::builder().uri("/api/daemon");
        if let Some(h) = host {
            b = b.header("host", h);
        }
        b.body(Body::empty()).unwrap()
    }

    #[tokio::test]
    async fn loopback_host_is_allowed() {
        for h in ["127.0.0.1:7700", "localhost:7700", "[::1]:7700"] {
            let resp = router(7700).oneshot(req_with_host(Some(h))).await.unwrap();
            assert_ne!(
                resp.status(),
                StatusCode::FORBIDDEN,
                "loopback host {h} must pass the guard"
            );
        }
    }

    #[tokio::test]
    async fn rebinding_host_is_refused() {
        // An attacker page whose domain resolved to 127.0.0.1 still sends its own Host.
        let resp = router(7700)
            .oneshot(req_with_host(Some("evil.example")))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn wrong_port_host_is_refused() {
        // Host for a different port isn't us — refuse.
        let resp = router(7700)
            .oneshot(req_with_host(Some("127.0.0.1:1234")))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn missing_host_is_refused() {
        let resp = router(7700).oneshot(req_with_host(None)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }
}
