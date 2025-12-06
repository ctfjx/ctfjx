use tokio::{
    sync::{mpsc, oneshot},
    time::Duration,
    time::timeout,
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

pub(crate) async fn send_syn(
    tx: mpsc::UnboundedSender<Message>,
    stream_id: StreamId,
) -> Result<usize, Error> {
    send_frame(tx, Frame::new_syn(stream_id)).await
}
pub(crate) async fn send_ack(
    tx: mpsc::UnboundedSender<Message>,
    stream_id: StreamId,
) -> Result<usize, Error> {
    send_frame(tx, Frame::new_ack(stream_id)).await
}
pub(crate) fn send_fin(
    tx: mpsc::UnboundedSender<Message>,
    stream_id: StreamId,
) -> Result<(), Error> {
    let frame = Frame::new_fin(stream_id);
    let (msg, _) = Message::new(frame);

    tx.send(msg)
        .map_err(|_| Error::MessageSendFail)
        .map_err(|_| Error::MessageSendFail)
}
pub(crate) async fn send_push(
    tx: mpsc::UnboundedSender<Message>,
    stream_id: StreamId,
    data: &[u8],
) -> Result<usize, Error> {
    send_frame(tx, Frame::new_push(stream_id, data)).await
}
