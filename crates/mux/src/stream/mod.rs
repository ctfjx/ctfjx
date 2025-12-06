use crate::{StreamId, error::Error, frame::Frame};
use bitflags::bitflags;
use parking_lot::RwLock;
use std::{
    cmp,
    pin::Pin,
    sync::OnceLock,
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    sync::{mpsc, oneshot},
};
use tokio_util::bytes::{Buf, Bytes};

pub(crate) mod allocate;
pub(crate) use allocate::*;
pub(crate) mod message;
pub(crate) use message::*;
pub(crate) mod manager;
pub(crate) use manager::*;

type FrameWriteFuture = Pin<Box<dyn Future<Output = Result<usize, Error>> + Send + Sync>>;

bitflags! {
    pub struct StreamPerms: u8 {
        const R = 1 << 0;
        const W = 1 << 1;
        const RW = Self::R.bits() | Self::W.bits();
    }
}

// closing:
// A -> FIN -> peer
// A denies w
// peer denies rw
// peer -> FIN -> A
// A denies rw
pub struct Stream {
    stream_id: StreamId,
    perms: RwLock<StreamPerms>,

    in_rx: mpsc::Receiver<Frame>,
    read_buf: Bytes,

    out_tx: mpsc::UnboundedSender<Message>,
    current_write_future: Option<FrameWriteFuture>,

    // for stream to internally send out when we loose rw perms
    trigger_close_tx: mpsc::UnboundedSender<StreamId>,
    // listen into when peer sends FIN
    peer_close_rx: oneshot::Receiver<()>,
    close_once: OnceLock<()>,
}

impl Stream {
    pub(crate) fn new(
        stream_id: StreamId,
        in_rx: mpsc::Receiver<Frame>,
        out_tx: mpsc::UnboundedSender<Message>,
        trigger_close_tx: mpsc::UnboundedSender<StreamId>,
        peer_close_rx: oneshot::Receiver<()>,
    ) -> Self {
        Self {
            stream_id,
            perms: RwLock::new(StreamPerms::RW),
            read_buf: Bytes::new(),
            current_write_future: None,
            in_rx,
            out_tx,
            trigger_close_tx,
            peer_close_rx,
            close_once: OnceLock::new(),
        }
    }

    // Should be used to send out a FIN
    pub fn close(&self) {
        self.close_once.get_or_init(|| {
            self.deny_perm(StreamPerms::W);
            let _ = message::send_fin(self.out_tx.clone(), self.stream_id);
        });
    }

    pub fn deny_perm(&self, perm: StreamPerms) {
        let mut p = self.perms.write();
        *p -= perm & StreamPerms::RW;

        if !p.contains(StreamPerms::RW) {
            let _ = self.trigger_close_tx.send(self.stream_id);
        }
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        if self.perms.read().contains(StreamPerms::W) {
            let _ = message::send_fin(self.out_tx.clone(), self.stream_id);
        }
        self.deny_perm(StreamPerms::RW);
    }
}

impl AsyncRead for Stream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let self_mut = self.get_mut();

        loop {
            if !self_mut.perms.read().contains(StreamPerms::R) {
                return Poll::Ready(Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "stream is closed",
                )));
            }

            // continue read op
            if !self_mut.read_buf.is_empty() {
                let cpy = cmp::min(self_mut.read_buf.len(), buf.remaining());
                buf.put_slice(&self_mut.read_buf[..cpy]);
                self_mut.read_buf.advance(cpy);
                return Poll::Ready(Ok(()));
            }

            match Pin::new(&mut self_mut.in_rx).poll_recv(cx) {
                Poll::Ready(Some(frame)) => {
                    self_mut.read_buf = Bytes::from(frame.payload);
                    continue;
                }
                Poll::Ready(None) => {
                    return Poll::Ready(Ok(()));
                }
                Poll::Pending => {
                    match Pin::new(&mut self_mut.peer_close_rx).poll(cx) {
                        Poll::Ready(_) => {
                            self_mut.deny_perm(StreamPerms::R);
                            return Poll::Ready(Ok(()));
                        }
                        Poll::Pending => (),
                    }
                    return Poll::Pending;
                }
            }
        }
    }
}

impl AsyncWrite for Stream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        if !self.perms.read().contains(StreamPerms::W) {
            return Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "stream is closed for writing",
            )));
        }

        if self.current_write_future.is_none() {
            let out_tx = self.out_tx.clone();
            let stream_id = self.stream_id;
            let data = buf.to_vec();

            self.current_write_future =
                Some(
                    Box::pin(async move { message::send_push(out_tx, stream_id, &data).await })
                        as FrameWriteFuture,
                );
            return Poll::Ready(Ok(buf.len()));
        }

        match self
            .current_write_future
            .as_mut()
            .unwrap()
            .as_mut()
            .poll(cx)
        {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(_)) => {
                let out_tx = self.out_tx.clone();
                let stream_id = self.stream_id;
                let data = buf.to_vec();

                self.current_write_future =
                    Some(
                        Box::pin(async move { message::send_push(out_tx, stream_id, &data).await })
                            as FrameWriteFuture,
                    );
                Poll::Ready(Ok(buf.len()))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(std::io::Error::other(e.to_string()))),
        }
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        match self.current_write_future.as_mut() {
            Some(fut) => match fut.as_mut().poll(cx) {
                Poll::Ready(Ok(_)) => {
                    self.current_write_future = None;
                    Poll::Ready(Ok(()))
                }
                Poll::Ready(Err(e)) => {
                    self.current_write_future = None;
                    Poll::Ready(Err(std::io::Error::other(e.to_string())))
                }
                Poll::Pending => Poll::Pending,
            },
            None => Poll::Ready(Ok(())),
        }
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        let res = match &mut self.current_write_future {
            Some(fut) => match fut.as_mut().poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(Ok(_)) => {
                    self.current_write_future = None;
                    Poll::Ready(Ok(()))
                }
                Poll::Ready(Err(e)) => {
                    self.current_write_future = None;
                    Poll::Ready(Err(std::io::Error::other(e.to_string())))
                }
            },
            None => Poll::Ready(Ok(())),
        };
        if !res.is_pending() {
            self.close();
        }
        res
    }
}
