use tokio_util::{
    bytes::{Buf, BufMut, BytesMut},
    codec::{Decoder, Encoder},
};

use crate::{VERSION_0, Version, error::Error, frame::*};

pub(crate) struct FrameCodec;

impl Encoder<Frame> for FrameCodec {
    type Error = Error;

    fn encode(&mut self, frame: Frame, buf: &mut BytesMut) -> Result<(), Self::Error> {
        let payload_len = frame.payload.len();
        if payload_len > u16::MAX as usize {
            return Err(Error::PayloadTooLong());
        }

        let frame_len = HEADER_LENGTH + payload_len;
        buf.reserve(frame_len);
        buf.put_u8(VERSION_0);
        buf.put_u8(frame.header.cmd as u8);
        buf.put_u16(frame.header.stream_id);
        buf.put_u16(payload_len as u16);
        buf.put_slice(&frame.payload);
        Ok(())
    }
}

impl Decoder for FrameCodec {
    type Item = Frame;
    type Error = Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if buf.len() < HEADER_LENGTH {
            return Ok(None);
        }

        let version = match Version::from(buf[0]) {
            0x0 => VERSION_0,
            n => return Err(Error::InvalidVersion(n)),
        };
        let cmd = Cmd::try_from(buf[1])?;
        let stream_id = (&buf[2..4]).get_u16();
        let payload_len = (&buf[4..6]).get_u16();

        let frame_len = HEADER_LENGTH + payload_len as usize;
        if buf.len() < frame_len {
            buf.reserve(frame_len - buf.len()); // make up the difference
            return Ok(None);
        }

        buf.advance(HEADER_LENGTH);
        Ok(Some(Frame {
            header: Header::new(version, cmd, stream_id, payload_len),
            payload: buf.split_to(payload_len as usize).to_vec(),
        }))
    }
}
