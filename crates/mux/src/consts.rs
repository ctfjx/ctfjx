use std::fmt::Display;

use crate::stream::{EVEN_STREAM_ID_START, ODD_STREAM_ID_START};

pub(crate) type Version = u8;
pub(crate) const VERSION_0: Version = 0x0;

pub(crate) type StreamId = u16;

#[derive(Clone)]
pub(crate) enum MultiplexerMode {
    Client,
    Server,
}
impl MultiplexerMode {
    pub fn get_starting_id(&self) -> StreamId {
        match self {
            MultiplexerMode::Client => ODD_STREAM_ID_START,
            MultiplexerMode::Server => EVEN_STREAM_ID_START,
        }
    }
}
impl Display for MultiplexerMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MultiplexerMode::Client => write!(f, "Client"),
            MultiplexerMode::Server => write!(f, "Server"),
        }
    }
}

pub(crate) const FRAME_BUFFER_SIZE: usize = 1 << 10; // 1kB
