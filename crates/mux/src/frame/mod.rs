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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_util::{
        bytes::BytesMut,
        codec::{Decoder, Encoder},
    };

    #[test]
    fn encode_decode_roundtrip() {
        let mut codec = FrameCodec;
        let mut buf = BytesMut::new();

        let frames = [
            Frame::new_syn(1),
            Frame::new_push(1, b"hello"),
            Frame::new_push(2, b"world"),
            Frame::new_fin(1),
        ];

        let mut decoded = Vec::new();

        for frame in frames {
            codec.encode(frame, &mut buf).unwrap();
            while let Some(f) = codec.decode(&mut buf).unwrap() {
                decoded.push(f);
            }
        }

        assert_eq!(decoded.len(), 4);
        assert_eq!(decoded[0].header.cmd, Cmd::Syn);
        assert_eq!(decoded[1].header.cmd, Cmd::Push);
        assert_eq!(decoded[1].payload, b"hello");
        assert_eq!(decoded[2].payload, b"world");
        assert_eq!(decoded[3].header.cmd, Cmd::Fin);
    }

    #[test]
    fn decode_partial_frame_returns_none() {
        let mut codec = FrameCodec;

        let frame = Frame::new_push(1, b"hello");
        let mut buf = BytesMut::new();
        codec.encode(frame, &mut buf).unwrap();

        buf.truncate(buf.len() - 1);

        let result = codec.decode(&mut buf).unwrap();
        assert!(result.is_none(), "Partial frame should return None");
    }
}
