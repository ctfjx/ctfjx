use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid cmd: {0}")]
    InvalidCmd(u8),
    #[error("invalid version: {0}")]
    InvalidVersion(u8),
    #[error("payload too large (>{})", u16::MAX)]
    PayloadTooLong(),

    #[error("message failed to send")]
    MessageSendFail,
    #[error("message took too long to send")]
    MessageSendTooLong,

    #[error("connection closed")]
    ConnectionClosed,

    #[error("exceeded max concurrent streams ({})", u16::MAX)]
    StreamLimitExceeded,
    #[error("duplicate stream id {0}")]
    DuplicateStream(u16),
    #[error("stream not found {0}")]
    StreamNotFound(u16),
    #[error("send frame failed for stream {0}")]
    SendFrameFailed(u16),

    #[error("internal: ")]
    Internal(String),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}
