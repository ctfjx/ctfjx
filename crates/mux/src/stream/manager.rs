use std::collections::HashMap;

use parking_lot::Mutex;
use tokio::sync::{mpsc, oneshot};

use crate::{
    StreamId,
    error::Error,
    frame::{Cmd, Frame},
};

pub(crate) struct StreamManager {
    streams: Mutex<HashMap<StreamId, StreamHandle>>,
    stream_creation_tx: mpsc::UnboundedSender<StreamId>,
    free_id_tx: mpsc::UnboundedSender<StreamId>,
}

pub(crate) struct StreamHandle {
    frame_tx: mpsc::Sender<Frame>,
    remote_fin_tx: Option<oneshot::Sender<()>>,
    remote_ack_tx: Option<oneshot::Sender<()>>,
    awaiting_fin: bool,
}

impl StreamManager {
    pub fn new(
        stream_creation_tx: mpsc::UnboundedSender<StreamId>,
        free_id_tx: mpsc::UnboundedSender<StreamId>,
    ) -> Self {
        Self {
            streams: Mutex::new(HashMap::new()),
            stream_creation_tx,
            free_id_tx,
        }
    }

    pub fn add_stream(
        &self,
        stream_id: StreamId,
        frame_tx: mpsc::Sender<Frame>,
        remote_fin_tx: oneshot::Sender<()>,
        remote_ack_tx: Option<oneshot::Sender<()>>,
    ) -> Result<(), Error> {
        let stream_handle = StreamHandle {
            frame_tx,
            remote_fin_tx: Some(remote_fin_tx),
            remote_ack_tx,
            awaiting_fin: false,
        };

        let mut streams = self.streams.lock();
        if streams.contains_key(&stream_id) {
            return Err(Error::DuplicateStream(stream_id));
        }

        streams.insert(stream_id, stream_handle);
        Ok(())
    }

    pub fn soft_remove_stream(&self, stream_id: StreamId) -> Result<(), Error> {
        let mut streams = self.streams.lock();
        if !streams.contains_key(&stream_id) {
            return Err(Error::StreamNotFound(stream_id));
        }
        streams
            .get_mut(&stream_id)
            .ok_or(Error::Internal("stream handle not found".to_string()))?
            .awaiting_fin = true;
        println!("stream marked as removed {stream_id}");
        Ok(())
    }

    pub async fn dispatch_frame(&self, frame: Frame) -> Result<(), Error> {
        let stream_id = frame.header.stream_id;
        println!(
            "{} got {}",
            stream_id,
            match frame.header.cmd {
                Cmd::Syn => "syn",
                Cmd::Ack => "ack",
                Cmd::Fin => "fin",
                Cmd::Push => "push",
            }
        );

        match frame.header.cmd {
            Cmd::Syn => self
                .stream_creation_tx
                .send(stream_id)
                .map_err(|_| Error::SendFrameFailed(stream_id)),
            Cmd::Ack => {
                let mut streams = self.streams.lock();
                let stream = streams
                    .get_mut(&stream_id)
                    .ok_or(Error::StreamNotFound(stream_id))?;

                if stream.awaiting_fin {
                    let _ = self.free_id_tx.send(stream_id);
                    streams.remove(&stream_id);
                    return Ok(());
                }

                stream
                    .remote_ack_tx
                    .take()
                    .ok_or(Error::Internal("remote ack tx not found".to_string()))?
                    .send(())
                    .map_err(|_| Error::SendFrameFailed(stream_id))
            }
            Cmd::Fin => {
                let mut streams = self.streams.lock();
                let stream = streams
                    .get_mut(&stream_id)
                    .ok_or(Error::StreamNotFound(stream_id))?;

                if stream.awaiting_fin {
                    return Ok(());
                }

                let _ = self.free_id_tx.send(stream_id);
                stream
                    .remote_fin_tx
                    .take()
                    .ok_or(Error::Internal("remote fin tx not found".to_string()))?
                    .send(())
                    .map_err(|_| Error::SendFrameFailed(stream_id))
            }
            Cmd::Push => {
                let frame_tx = self
                    .streams
                    .lock()
                    .get(&stream_id)
                    .map(|s| s.frame_tx.clone())
                    .ok_or(Error::StreamNotFound(stream_id))?;
                frame_tx
                    .send(frame)
                    .await
                    .map_err(|_| Error::SendFrameFailed(stream_id))
            }
        }
    }
}
