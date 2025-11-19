use std::{
    collections::VecDeque,
    sync::{
        Mutex,
        atomic::{AtomicU16, Ordering},
    },
};

use crate::{StreamId, error::Error};

pub(crate) const ODD_STREAM_ID_START: StreamId = 0x01;
pub(crate) const EVEN_STREAM_ID_START: StreamId = 0x02;

pub(crate) struct StreamIdAllocator {
    curr: AtomicU16,
    free_list: Mutex<VecDeque<StreamId>>,
}

impl StreamIdAllocator {
    pub fn new(start: StreamId) -> Self {
        Self {
            curr: AtomicU16::new(start),
            free_list: Mutex::new(VecDeque::new()),
        }
    }

    pub fn alloc(&self) -> Result<StreamId, Error> {
        if let Some(id) = self.free_list.lock().unwrap().pop_front() {
            return Ok(id);
        }

        let curr = self.curr.load(Ordering::Relaxed);
        if curr > u16::MAX - 2 {
            return Err(Error::StreamLimitExceeded);
        }

        Ok(self.curr.fetch_add(2, Ordering::Relaxed))
    }

    pub fn free(&self, stream_id: StreamId) {
        let mut free_list = self.free_list.lock().unwrap();
        free_list.push_back(stream_id);
    }
}
