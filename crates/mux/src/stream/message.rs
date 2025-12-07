use tokio::{
    runtime,
    sync::{mpsc, oneshot},
    time::{Duration, timeout},
};

use crate::{StreamId, error::Error, frame::Frame};

/// Message is used to send out [`Cmd::Push`] frames
pub(crate) struct Message {
    pub(crate) frame: Frame,
    // marks when the message has been sent out or failed to be sent
    pub(crate) done_tx: oneshot::Sender<Result<usize, Error>>,
}

impl Message {
    pub fn new(frame: Frame) -> (Self, oneshot::Receiver<Result<usize, Error>>) {
        let (tx, rx) = oneshot::channel();
        (Self { frame, done_tx: tx }, rx)
    }
}

async fn send_frame(tx: mpsc::UnboundedSender<Message>, frame: Frame) -> Result<usize, Error> {
    let (msg, rx) = Message::new(frame);
    tx.send(msg).map_err(|_| Error::MessageSendFail)?;

    timeout(Duration::from_secs(5), rx)
        .await
        .map_err(|_| Error::MessageSendTooLong)?
        .map_err(|_| Error::MessageSendFail)?
}

macro_rules! sender {
    ($frame_t:ident) => {
        paste::paste! {
            #[allow(unused)]
            pub(crate) async fn [<send_ $frame_t>](
                tx: mpsc::UnboundedSender<Message>,
                stream_id: StreamId,
            ) -> Result<usize, Error> {
                send_frame(tx, Frame::[<new_ $frame_t>](stream_id)).await
            }

            #[allow(unused)]
            pub(crate) fn [<send_ $frame_t _sync>](
                tx: mpsc::UnboundedSender<Message>,
                stream_id: StreamId,
            ) -> Result<usize, Error> {
                tokio::runtime::Handle::current()
                    .block_on(send_frame(tx, Frame::[<new_ $frame_t>](stream_id)))
            }
        }
    };

    ($frame_t:ident, $( $arg_name:ident : $arg_ty:ty ),+ ) => {
        paste::paste! {
            #[allow(unused)]
            pub(crate) async fn [<send_ $frame_t>](
                tx: mpsc::UnboundedSender<Message>,
                stream_id: StreamId,
                $( $arg_name : $arg_ty ),+
            ) -> Result<usize, Error> {
                send_frame(
                    tx,
                    Frame::[<new_ $frame_t>](stream_id, $( $arg_name ),+)
                ).await
            }

            #[allow(unused)]
            pub(crate) fn [<send_ $frame_t _sync>](
                tx: mpsc::UnboundedSender<Message>,
                stream_id: StreamId,
                $( $arg_name : $arg_ty ),+
            ) -> Result<usize, Error> {
                tokio::runtime::Handle::current()
                    .block_on(send_frame(
                        tx,
                        Frame::[<new_ $frame_t>](stream_id, $( $arg_name ),+)
                    ))
            }
        }
    };
}

sender!(syn);
sender!(fin);
sender!(ack);
sender!(push, data: &[u8]);
