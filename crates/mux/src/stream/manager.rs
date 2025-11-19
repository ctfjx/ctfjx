use std::collections::HashMap;

use parking_lot::Mutex;

use crate::StreamId;

pub(crate) struct StreamHandle {}

pub(crate) struct StreamManager {
    streams: Mutex<HashMap<StreamId, StreamHandle>>,
}
