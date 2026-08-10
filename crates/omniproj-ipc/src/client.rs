//! Client side: one round-trip per connection.

use std::io;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use crate::proto::{Request, Response};
use crate::socket_path;

/// Send one request to the default socket and read the response. Errors if the daemon
/// isn't listening (socket missing or connection refused) — the caller (CLI) treats
/// that as "daemon down" and lazy-starts.
pub async fn request(req: &Request) -> io::Result<Response> {
    let mut stream = UnixStream::connect(socket_path()).await?;
    let bytes = serde_json::to_vec(req)?;
    stream.write_all(&bytes).await?;
    stream.shutdown().await?; // half-close write so the server reads to EOF

    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await?;
    let resp: Response =
        serde_json::from_slice(&buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(resp)
}

/// True if a daemon is alive and answering on the socket.
pub async fn ping() -> bool {
    matches!(request(&Request::Ping).await, Ok(Response::Pong { .. }))
}
