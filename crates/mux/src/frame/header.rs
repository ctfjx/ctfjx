use crate::{
    consts::{StreamId, Version},
    error::Error,
};
use std::mem;

pub(crate) const HEADER_LENGTH: usize = mem::size_of::<Header>();
pub(crate) type DataLength = u16;

/// version - 1 byte
/// cmd - 1 byte
/// stream_id - 2 bytes
/// payload_len - 2 bytes
#[derive(Debug, Clone)]
#[repr(C)]
pub(crate) struct Header {
    pub version: Version,
    pub cmd: Cmd,
    pub stream_id: StreamId,
    pub payload_len: DataLength,
}

impl Header {
    pub fn new(version: Version, cmd: Cmd, stream_id: StreamId, payload_len: DataLength) -> Self {
        Self {
            version,
            cmd,
            stream_id,
            payload_len,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum Cmd {
    /// To establish a new stream
    Syn = 0x01,
    /// To acknowledge a received frame
    Ack = 0x02,
    /// To close a stream
    Fin = 0x03,
    /// To send a payload
    Push = 0x04,
}

impl TryFrom<u8> for Cmd {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(Cmd::Syn),
            0x02 => Ok(Cmd::Ack),
            0x03 => Ok(Cmd::Fin),
            0x04 => Ok(Cmd::Push),
            _ => Err(Error::InvalidCmd(value)),
        }
    }
}
