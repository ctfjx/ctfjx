use crate::{StreamId, error::Error, frame::Frame};
use std::{
    cmp,
    pin::Pin,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    sync::{broadcast, mpsc, oneshot},
};
use tokio_util::bytes::{Buf, Bytes};

pub(crate) mod message;
pub(crate) use message::*;

type FrameWriteFuture = Pin<Box<dyn Future<Output = Result<usize, Error>> + Send + Sync>>;

pub struct Stream {
    stream_id: StreamId,

    in_rx: mpsc::UnboundedReceiver<Frame>,
    read_open: AtomicBool, // assume if read=false, stream is closed
    read_buf: Bytes,

    out_tx: mpsc::UnboundedSender<Message>,
    write_open: AtomicBool,
    current_write_future: Option<FrameWriteFuture>,

    peer_close_rx: oneshot::Receiver<()>,

    shutdown_rx: broadcast::Receiver<()>, // triggered when raw conn is closed
    close_once: OnceLock<()>,
}

impl Stream {
    pub fn new(
        stream_id: StreamId,
        in_rx: mpsc::UnboundedReceiver<Frame>,
        out_tx: mpsc::UnboundedSender<Message>,
        peer_close_rx: oneshot::Receiver<()>,
        shutdown_rx: broadcast::Receiver<()>,
    ) -> Self {
        Self {
            stream_id,
            read_buf: Bytes::new(),
            current_write_future: None,
            in_rx,
            out_tx,
            read_open: AtomicBool::new(true),
            write_open: AtomicBool::new(true),
            shutdown_rx,
            peer_close_rx,
            close_once: OnceLock::new(),
        }
    }

    pub fn close(&self) {
        self.close_once.get_or_init(|| {
            self.write_open.store(false, Ordering::Release);
            let _ = message::send_fin(self.out_tx.clone(), self.stream_id);
        });
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        if self.write_open.load(Ordering::Acquire) {
            let _ = message::send_fin(self.out_tx.clone(), self.stream_id);
        }
        self.write_open.store(false, Ordering::Release);
        self.read_open.store(false, Ordering::Release);
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
            if !self_mut.read_open.load(Ordering::Acquire) {
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
                            self_mut.read_open.store(false, Ordering::Release);
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
        if !self.write_open.load(Ordering::Acquire) {
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
