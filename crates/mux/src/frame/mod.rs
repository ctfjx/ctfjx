pub mod header;
pub use header::*;

pub mod codec;
pub use codec::*;

use crate::{StreamId, VERSION_0};

#[derive(Debug)]
pub(crate) struct Frame {
    pub header: Header,
    pub payload: Vec<u8>,
}

impl Frame {
    // Frame length in bytes
    pub fn len(&self) -> usize {
        self.header.payload_len as usize + HEADER_LENGTH
    }

    // To open a stream
    pub fn new_syn(stream_id: StreamId) -> Self {
        Self {
            header: Header::new(VERSION_0, Cmd::Syn, stream_id, 0),
            payload: vec![],
        }
    }

    // To close a stream
    pub fn new_fin(stream_id: StreamId) -> Self {
        Self {
            header: Header::new(VERSION_0, Cmd::Fin, stream_id, 0),
            payload: vec![],
        }
    }

    // To ack a new frame
    pub fn new_ack(stream_id: StreamId) -> Self {
        Self {
            header: Header::new(VERSION_0, Cmd::Ack, stream_id, 0),
            payload: vec![],
        }
    }

    // To send data
    pub fn new_push(stream_id: StreamId, data: &[u8]) -> Self {
        Self {
            header: Header::new(VERSION_0, Cmd::Push, stream_id, data.len() as u16),
            payload: data.to_vec(),
        }
    }
}
