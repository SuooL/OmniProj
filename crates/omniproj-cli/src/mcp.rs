//! MCP stdio server (benchmark review P0#1 — the consumption loop).
//!
//! Exposes the distilled project memory to MCP clients (Claude Code et al.) so an
//! agent can pull re-entry context at session start instead of a human pasting it.
//! Read-only over `~/.omniproj`; no LLM; resolves "which project" from the process cwd
//! (Claude Code spawns stdio servers with cwd = the project root).
//!
//! Hand-rolled JSON-RPC per the documented decision (same precedent as `omniproj-ipc`
//! over tonic): the needed surface is 7 methods, stable across every protocol
//! revision to date. Wire rules that MUST hold (see task research):
//!  - newline-delimited JSON-RPC 2.0 (NOT LSP Content-Length framing);
//!  - stdout carries protocol messages ONLY — all logging goes to stderr;
//!  - notifications (no `id`) are never answered;
//!  - exit on stdin EOF (client disconnected).

use anyhow::Result;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// Protocol revisions we know; `initialize` echoes the client's version when known,
/// else falls back to a safe one. Adding a future revision is a one-line change.
const KNOWN_VERSIONS: [&str; 4] = ["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"];
const FALLBACK_VERSION: &str = "2025-06-18";

/// The state files a project exposes as resources, in re-entry-priority order.
const RESOURCE_KINDS: [&str; 5] = ["briefing", "decisions", "open", "opinion", "learned"];

type RpcResult = Result<Value, (i64, String)>;

/// Serve MCP over stdio until the client disconnects (stdin EOF).
pub async fn serve() -> Result<()> {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut out = tokio::io::stdout();
    eprintln!("[omniproj-mcp] serving (cwd = {})", cwd_display());
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let Some(resp) = handle_line(&line) else {
            continue; // notification or unanswerable garbage — never respond
        };
        out.write_all(serde_json::to_string(&resp)?.as_bytes())
            .await?;
        out.write_all(b"\n").await?;
        out.flush().await?;
    }
    eprintln!("[omniproj-mcp] stdin closed — shutting down");
    Ok(())
}

/// One message in, at most one response out. `None` = don't respond (notifications;
/// also unparseable lines without a recoverable id, where a null-id parse error is
/// the best JSON-RPC allows — we send that only for syntactically broken requests).
fn handle_line(line: &str) -> Option<Value> {
    let msg: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => {
            return Some(error_resp(Value::Null, -32700, "parse error"));
        }
    };
    let id = msg.get("id").cloned()?; // no id → notification → no response
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    let result = dispatch(method, &params);
    Some(match result {
        Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
        Err((code, m)) => error_resp(id, code, &m),
    })
}

fn error_resp(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

fn dispatch(method: &str, params: &Value) -> RpcResult {
    match method {
        "initialize" => handle_initialize(params),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(tools_list()),
        "tools/call" => handle_tools_call(params),
        "resources/list" => Ok(resources_list()),
        "resources/read" => handle_resources_read(params),
        _ => Err((-32601, format!("method not found: {method}"))),
    }
}

fn handle_initialize(params: &Value) -> RpcResult {
    let client_version = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or(FALLBACK_VERSION);
    let version = if KNOWN_VERSIONS.contains(&client_version) {
        client_version
    } else {
        FALLBACK_VERSION
    };
    Ok(json!({
        "protocolVersion": version,
        "capabilities": {"tools": {}, "resources": {}},
        "serverInfo": {"name": "omniproj", "version": env!("CARGO_PKG_VERSION")},
    }))
}

// ------------------------------------------------------------------------ project state

fn cwd_display() -> String {
    std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "?".into())
}

/// The registered project owning the server's cwd, if any.
fn cwd_project() -> Option<omniproj_core::ProjectMeta> {
    let cwd = std::env::current_dir().ok()?;
    let canon = std::fs::canonicalize(&cwd).unwrap_or(cwd);
    omniproj_core::find_by_cwd(&canon)
}

/// Read one state file kind for a project. `None` when absent/empty.
fn state_file(hash: &str, kind: &str) -> Option<String> {
    let path = if kind == "learned" {
        omniproj_core::learned_path(hash)
    } else {
        omniproj_core::auto_dir(hash).join(format!("{kind}.md"))
    };
    std::fs::read_to_string(path)
        .ok()
        .filter(|s| !s.trim().is_empty())
}

/// The re-entry payload `project_recall` returns: briefing + open + decisions,
/// clearly delimited, with provenance up top. No LLM — this is the stored state.
fn recall_payload(meta: &omniproj_core::ProjectMeta) -> String {
    let mut out = format!(
        "# OmniProj recall — {} ({})\nlast distilled: {}\n",
        meta.name,
        meta.path,
        meta.last_distilled.as_deref().unwrap_or("never"),
    );
    for kind in ["briefing", "open", "decisions"] {
        if let Some(text) = state_file(&meta.hash, kind) {
            out.push_str(&format!("\n## {kind}\n{}\n", text.trim()));
        }
    }
    out
}

// ------------------------------------------------------------------------------- tools

fn tools_list() -> Value {
    json!({"tools": [{
        "name": "project_search",
        "description": "Full-text search across this project's captured agent \
                        sessions (FTS5, local, no LLM). Finds past discussions, \
                        decisions and errors by literal text (CJK supported).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "Text to search for."},
                "limit": {"type": "integer", "description": "Max hits (default 10)."}
            },
            "required": ["query"]
        }
    }, {
        "name": "project_recall",
        "description": "Recall OmniProj's distilled memory for the current project: the \
                        re-entry briefing, open threads, and decision log (read-only, \
                        no LLM call). Use at the start of a session to know where the \
                        project stands.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Project directory (defaults to the server's cwd)."
                }
            }
        }
    }]})
}

fn handle_tools_call(params: &Value) -> RpcResult {
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    match name {
        "project_recall" => {
            let meta = match params
                .get("arguments")
                .and_then(|a| a.get("path"))
                .and_then(Value::as_str)
            {
                Some(p) => {
                    let canon =
                        std::fs::canonicalize(p).unwrap_or_else(|_| std::path::PathBuf::from(p));
                    omniproj_core::find_by_cwd(&canon)
                }
                None => cwd_project(),
            };
            let Some(meta) = meta else {
                return Ok(tool_text(
                    "No OmniProj project registered for this directory. \
                     Run `omniproj add <repo> && omniproj briefing` first.",
                    true,
                ));
            };
            Ok(tool_text(&recall_payload(&meta), false))
        }
        "project_search" => {
            let args = params.get("arguments").cloned().unwrap_or(Value::Null);
            let query = args.get("query").and_then(Value::as_str).unwrap_or("");
            if query.is_empty() {
                return Err((-32602, "project_search requires a query".into()));
            }
            let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(10) as usize;
            let Ok(cwd) = std::env::current_dir() else {
                return Err((-32603, "cannot resolve cwd".into()));
            };
            match omniproj_index::search_project(&cwd, query, limit) {
                Ok(hits) if hits.is_empty() => Ok(tool_text("no matches", false)),
                Ok(hits) => {
                    let lines: Vec<String> = hits
                        .iter()
                        .map(|h| {
                            format!("[{} {}] {}", h.source, h.role, h.snippet.replace('\n', " "))
                        })
                        .collect();
                    Ok(tool_text(&lines.join("\n"), false))
                }
                Err(e) => Ok(tool_text(&format!("search failed: {e:#}"), true)),
            }
        }
        other => Err((-32602, format!("unknown tool: {other}"))),
    }
}

fn tool_text(text: &str, is_error: bool) -> Value {
    json!({"content": [{"type": "text", "text": text}], "isError": is_error})
}

// --------------------------------------------------------------------------- resources

fn resources_list() -> Value {
    let resources: Vec<Value> = match cwd_project() {
        Some(meta) => RESOURCE_KINDS
            .iter()
            .filter(|kind| state_file(&meta.hash, kind).is_some())
            .map(|kind| {
                json!({
                    "uri": format!("omniproj://project/{kind}"),
                    "name": format!("{} — {}", meta.name, kind),
                    "mimeType": "text/markdown",
                })
            })
            .collect(),
        None => Vec::new(),
    };
    json!({"resources": resources})
}

fn handle_resources_read(params: &Value) -> RpcResult {
    let uri = params.get("uri").and_then(Value::as_str).unwrap_or("");
    let kind = uri.strip_prefix("omniproj://project/").unwrap_or("");
    if !RESOURCE_KINDS.contains(&kind) {
        return Err((-32602, format!("unknown resource uri: {uri}")));
    }
    let Some(meta) = cwd_project() else {
        return Err((-32602, "no registered project for this directory".into()));
    };
    let Some(text) = state_file(&meta.hash, kind) else {
        return Err((-32602, format!("resource is empty/absent: {uri}")));
    };
    Ok(json!({"contents": [{"uri": uri, "mimeType": "text/markdown", "text": text}]}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(id: u64, method: &str, params: Value) -> String {
        json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string()
    }

    #[test]
    fn initialize_echoes_known_version_and_falls_back_for_unknown() {
        let resp = handle_line(&req(
            0,
            "initialize",
            json!({"protocolVersion": "2025-11-25"}),
        ))
        .unwrap();
        assert_eq!(resp["result"]["protocolVersion"], "2025-11-25");
        assert_eq!(resp["id"], 0);
        assert_eq!(resp["result"]["serverInfo"]["name"], "omniproj");

        let resp = handle_line(&req(
            1,
            "initialize",
            json!({"protocolVersion": "2099-01-01"}),
        ))
        .unwrap();
        assert_eq!(resp["result"]["protocolVersion"], FALLBACK_VERSION);
    }

    #[test]
    fn notifications_get_no_response() {
        let n = json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string();
        assert!(handle_line(&n).is_none());
        let n = json!({"jsonrpc": "2.0", "method": "notifications/cancelled"}).to_string();
        assert!(handle_line(&n).is_none());
    }

    #[test]
    fn unknown_method_is_32601_and_id_is_echoed() {
        let resp = handle_line(&req(7, "prompts/list", json!({}))).unwrap();
        assert_eq!(resp["error"]["code"], -32601);
        assert_eq!(resp["id"], 7);
    }

    #[test]
    fn parse_error_is_32700() {
        let resp = handle_line("{not json").unwrap();
        assert_eq!(resp["error"]["code"], -32700);
    }

    #[test]
    fn ping_returns_empty_object_and_tools_list_has_schema() {
        let resp = handle_line(&req(2, "ping", json!({}))).unwrap();
        assert_eq!(resp["result"], json!({}));

        let resp = handle_line(&req(3, "tools/list", json!({}))).unwrap();
        let tools = resp["result"]["tools"].as_array().unwrap();
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"project_recall") && names.contains(&"project_search"));
        for t in tools {
            assert_eq!(t["inputSchema"]["type"], "object");
        }
    }

    #[test]
    fn unknown_tool_and_bad_resource_are_32602() {
        let resp = handle_line(&req(
            4,
            "tools/call",
            json!({"name": "nope", "arguments": {}}),
        ))
        .unwrap();
        assert_eq!(resp["error"]["code"], -32602);

        let resp =
            handle_line(&req(5, "resources/read", json!({"uri": "omniproj://x/y"}))).unwrap();
        assert_eq!(resp["error"]["code"], -32602);
    }
}
