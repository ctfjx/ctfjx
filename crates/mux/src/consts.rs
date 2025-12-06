pub(crate) type Version = u8;
pub(crate) const VERSION_0: Version = 0x0;

pub(crate) type StreamId = u16;
pub(crate) enum MultiplexerMode {
    Client,
    Server,
}
