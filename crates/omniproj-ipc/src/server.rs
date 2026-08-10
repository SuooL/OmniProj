//! Server side: bind the socket and frame one request/response per connection. The
//! accept loop itself lives in the daemon (so it can close over the daemon's shared
//! state); this module just owns the socket lifecycle and framing.

use std::io;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

use crate::proto::{Request, Response};
use crate::socket_path;

/// Bind the daemon socket, removing any stale file first (a previous daemon that
/// didn't clean up — the new instance already holds the single-instance flock, so the
/// old socket is dead). Call [`cleanup`] on shutdown.
pub fn bind() -> io::Result<UnixListener> {
    let path = socket_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Unix sockets can't be re-bound while the file exists; unlink the stale one.
    let _ = std::fs::remove_file(&path);
    UnixListener::bind(&path)
}

/// Remove the socket file. Best-effort; safe to call on shutdown.
pub fn cleanup() {
    let _ = std::fs::remove_file(socket_path());
}

/// Read the single request a client sent before half-closing its write half.
pub async fn read_request(stream: &mut UnixStream) -> io::Result<Request> {
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await?;
    serde_json::from_slice(&buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Write the response and flush (the connection closes on drop).
pub async fn write_response(stream: &mut UnixStream, resp: &Response) -> io::Result<()> {
    let bytes = serde_json::to_vec(resp)?;
    stream.write_all(&bytes).await?;
    stream.flush().await
}
