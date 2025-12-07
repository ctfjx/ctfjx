use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt},
    select,
    sync::{broadcast, mpsc},
};
use tokio_util::codec::{FramedRead, FramedWrite};

use crate::{
    StreamId,
    frame::FrameCodec,
    stream::{Message, StreamIdAllocator, StreamManager},
};

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

pub(crate) async fn ingress_frame_dispatcher(
    mut conn: impl AsyncRead + Unpin,
    stream_manager: Arc<StreamManager>,
    mut shutdown_rx: broadcast::Receiver<()>,
) {
    let mut r = FramedRead::new(&mut conn, FrameCodec);
    loop {
        select! {
            frame = r.next() => {
                match frame {
                    Some(Ok(frame)) => {
                        let _ = stream_manager.dispatch_frame(frame).await;
                    }
                    None => {
                        return;
                    }
                    Some(Err(_)) => {
                        return;
                    }
                }
            }

            _ = shutdown_rx.recv() => {
                return;
            }
        }
    }
}

pub(crate) async fn stream_close_handle(
    mut close_rx: mpsc::UnboundedReceiver<StreamId>,
    mut free_id_rx: mpsc::UnboundedReceiver<StreamId>,
    stream_manager: Arc<StreamManager>,
    id_authority: Arc<StreamIdAllocator>,
    mut shutdown_rx: broadcast::Receiver<()>,
) {
    loop {
        select! {
            Some(stream_id) = free_id_rx.recv() => {
                println!("stream close handle id {stream_id}");
                id_authority.free(stream_id);
            }

            Some(stream_id) = close_rx.recv() => {
                let _ =  stream_manager.soft_remove_stream(stream_id);
            }

            _ = shutdown_rx.recv() => {
                return;
            }
        }
    }
}
