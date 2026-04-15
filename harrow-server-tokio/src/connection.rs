use std::sync::Arc;

use tokio::net::TcpStream;

use harrow_core::dispatch::SharedState;

use crate::ServerConfig;
use crate::h1::dispatcher;

pub(crate) async fn handle_tcp_connection(
    stream: TcpStream,
    shared: Arc<SharedState>,
    config: &ServerConfig,
    shutdown: harrow_server::ShutdownSignal,
) -> Result<(), Box<dyn std::error::Error>> {
    stream.set_nodelay(true)?;
    dispatcher::handle_connection_with_shutdown(stream, shared, config, &shutdown).await
}

#[doc(hidden)]
pub async fn handle_connection<S>(
    stream: S,
    shared: Arc<SharedState>,
    config: &ServerConfig,
) -> Result<(), Box<dyn std::error::Error>>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let shutdown = harrow_server::ShutdownSignal::new();
    dispatcher::handle_connection_with_shutdown(stream, shared, config, &shutdown).await
}
