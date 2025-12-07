use mux::Multiplexer;
use tokio::io::{DuplexStream, duplex};

/// returns (client, server)
pub fn make_mux_pair() -> (Multiplexer<DuplexStream>, Multiplexer<DuplexStream>) {
    let (client, server) = duplex(64 * 1024);

    (Multiplexer::client(client), Multiplexer::server(server))
}
