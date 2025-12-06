use std::{
    marker::PhantomData,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use parking_lot::Once;
use tokio::{
    io::{self, AsyncRead, AsyncWrite},
    sync::{broadcast, mpsc, oneshot},
};

use crate::{
    MultiplexerMode, Stream, StreamId, consts,
    error::Error,
    poll,
    stream::{
        self, EVEN_STREAM_ID_START, Message, ODD_STREAM_ID_START, StreamIdAllocator, StreamManager,
    },
};

pub struct Multiplexer<T: AsyncRead + AsyncWrite + Send + Unpin + 'static> {
    id_ca: Arc<StreamIdAllocator>,
    stream_manager: Arc<StreamManager>,

    create_stream_rx: tokio::sync::Mutex<mpsc::UnboundedReceiver<StreamId>>,

    shutdown_tx: broadcast::Sender<()>,
    shutdown_once: Once,
    is_shutdown: AtomicBool,

    msg_tx: mpsc::UnboundedSender<Message>,
    close_tx: mpsc::UnboundedSender<StreamId>,
    _phantom: PhantomData<T>,
}

impl<T: AsyncRead + AsyncWrite + Send + Unpin + 'static> Multiplexer<T> {
    fn new(conn: T, mode: MultiplexerMode) -> Self {
        let (conn_reader, conn_writer) = io::split(conn);
        let (msg_tx, msg_rx) = mpsc::unbounded_channel();
        let (close_tx, close_rx) = mpsc::unbounded_channel();
        let (stream_creation_tx, stream_creation_rx) = mpsc::unbounded_channel();

        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
        let shutdown_rx2 = shutdown_tx.subscribe();
        let shutdown_rx3 = shutdown_tx.subscribe();

        let session = Self {
            id_ca: Arc::new(StreamIdAllocator::new(match mode {
                MultiplexerMode::Client => ODD_STREAM_ID_START,
                MultiplexerMode::Server => EVEN_STREAM_ID_START,
            })),
            stream_manager: Arc::new(StreamManager::new(stream_creation_tx)),
            create_stream_rx: tokio::sync::Mutex::new(stream_creation_rx),
            shutdown_tx,
            shutdown_once: Once::new(),
            is_shutdown: AtomicBool::new(false),
            msg_tx,
            close_tx,
            _phantom: PhantomData,
        };

        tokio::spawn(poll::egress_message_dispatcher(
            msg_rx,
            conn_writer,
            shutdown_rx,
        ));
        tokio::spawn(poll::ingress_frame_dispatcher(
            conn_reader,
            session.stream_manager.clone(),
            shutdown_rx2,
        ));
        tokio::spawn(poll::stream_close_handle(
            close_rx,
            session.stream_manager.clone(),
            session.id_ca.clone(),
            shutdown_rx3,
        ));

        session
    }

    pub fn server(conn: T) -> Self {
        Self::new(conn, MultiplexerMode::Server)
    }
    pub fn client(conn: T) -> Self {
        Self::new(conn, MultiplexerMode::Client)
    }

    pub async fn open(&self) -> Result<Stream, Error> {
        if self.is_shutdown.load(Ordering::SeqCst) {
            return Err(Error::ConnectionClosed);
        }

        let close_tx = self.close_tx.clone();
        let msg_tx = self.msg_tx.clone();
        let shutdown_rx = self.shutdown_tx.subscribe();
        let stream_id = self.id_ca.alloc()?;
        let (in_tx, in_rx) = mpsc::channel(consts::FRAME_BUFFER_SIZE);
        let (peer_close_tx, peer_close_rx) = oneshot::channel();

        let stream = Stream::new(
            stream_id,
            in_rx,
            msg_tx.clone(),
            shutdown_rx,
            close_tx,
            peer_close_rx,
        );

        let (peer_ack_tx, peer_ack_rx) = oneshot::channel();
        self.stream_manager
            .add_stream(stream_id, in_tx, peer_close_tx, Some(peer_ack_tx))?;

        stream::send_syn(msg_tx, stream_id).await?;
        peer_ack_rx
            .await
            .map_err(|_| Error::Internal("peer ack not found".to_string()))?;

        Ok(stream)
    }

    /// accept a connection
    pub async fn accept(&self) -> Result<Stream, Error> {
        let stream_id = self
            .create_stream_rx
            .lock()
            .await
            .recv()
            .await
            .ok_or(Error::ConnectionClosed)?;

        let shutdown_rx = self.shutdown_tx.subscribe();
        let close_tx = self.close_tx.clone();
        let msg_tx = self.msg_tx.clone();
        let (frame_tx, frame_rx) = mpsc::channel(consts::FRAME_BUFFER_SIZE);
        let (peer_close_tx, peer_close_rx) = oneshot::channel();

        let stream = Stream::new(
            stream_id,
            frame_rx,
            msg_tx,
            shutdown_rx,
            close_tx,
            peer_close_rx,
        );
        self.stream_manager
            .add_stream(stream_id, frame_tx, peer_close_tx, None)?;
        stream::send_ack(self.msg_tx.clone(), stream_id).await?;

        Ok(stream)
    }

    /// Close the session
    ///
    /// This method gracefully closes the session, including:
    /// - Setting the shutdown flag
    /// - Notifying all background tasks to stop
    /// - Closing the underlying connection
    ///
    /// Note: After calling this method, the session can no longer be used to send data.
    pub fn close(self) {
        self.shutdown_once.call_once(|| {
            self.is_shutdown.store(true, Ordering::SeqCst);
            let _ = self.shutdown_tx.send(());
        });
    }
}
