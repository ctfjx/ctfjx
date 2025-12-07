use std::{
    collections::VecDeque,
    sync::{
        Mutex,
        atomic::{AtomicU16, Ordering},
    },
};

use crate::{MultiplexerMode, StreamId, error::Error};

pub(crate) const ODD_STREAM_ID_START: StreamId = 0x01;
pub(crate) const EVEN_STREAM_ID_START: StreamId = 0x02;

pub(crate) struct StreamIdAllocator {
    mode: MultiplexerMode,
    curr: AtomicU16,
    free_list: Mutex<VecDeque<StreamId>>,
}

impl StreamIdAllocator {
    pub(crate) fn new(m: &MultiplexerMode) -> Self {
        let start = m.get_starting_id();
        Self {
            mode: m.clone(),
            curr: AtomicU16::new(start),
            free_list: Mutex::new(VecDeque::new()),
        }
    }

    pub(crate) fn alloc(&self) -> Result<StreamId, Error> {
        if let Some(id) = self.free_list.lock().unwrap().pop_front() {
            println!("alloc from free list {id}");
            return Ok(id);
        }

        let curr = self.curr.load(Ordering::Relaxed);
        if curr > u16::MAX - 2 {
            return Err(Error::StreamLimitExceeded);
        }

        let id = self.curr.fetch_add(2, Ordering::Relaxed);
        println!("alloc from inc {id}");
        Ok(id)
    }

    pub(crate) fn free(&self, stream_id: StreamId) {
        let is_odd = ((stream_id >> 1) << 1) != stream_id;
        let starts_odd = self.mode.get_starting_id() == ODD_STREAM_ID_START;
        if starts_odd != is_odd {
            return;
        }

        let mut free_list = self.free_list.lock().unwrap();
        println!("freed {stream_id}");
        free_list.push_back(stream_id);
    }
}
