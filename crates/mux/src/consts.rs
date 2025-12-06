pub(crate) type Version = u8;
pub(crate) const VERSION_0: Version = 0x0;

pub(crate) type StreamId = u16;
pub(crate) enum MultiplexerMode {
    Client,
    Server,
}

pub(crate) const FRAME_BUFFER_SIZE: usize = 1 << 10; // 1kB
