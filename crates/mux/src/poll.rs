use futures_util::SinkExt;
use tokio::{
    io::{AsyncWrite, AsyncWriteExt},
    select,
    sync::{broadcast, mpsc},
};
use tokio_util::codec::FramedWrite;

use crate::{frame::FrameCodec, stream::Message};

pub(crate) async fn egress_message_dispatcher(
    mut msg_rx: mpsc::UnboundedReceiver<Message>,
    mut conn: impl AsyncWrite + Unpin,
    mut shutdown_rx: broadcast::Receiver<()>,
) {
    let mut w = FramedWrite::new(&mut conn, FrameCodec);
    loop {
        select! {
            msg = msg_rx.recv() => {
                match msg {
                    Some(msg) => {
                        let bytes_written = msg.frame.len();
                        let _ = msg.done_tx.send(w.send(msg.frame).await.map(|_| bytes_written));
                    }
                    None => {
                        break;
                    }
                }
            }

            _ = shutdown_rx.recv() => {
                drop(msg_rx);
                let _ = conn.shutdown().await;
                return;
            }
        }
    }

    let _ = conn.shutdown().await;
}
