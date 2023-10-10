use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};

pub struct Message {
    pub addr: SocketAddr,
    pub data: Vec<u8>,
}

#[derive(Clone)]
pub struct MessageQueue {
    queue: Arc<Mutex<Vec<Message>>>,
}

#[allow(dead_code)]
impl MessageQueue {
    pub fn new() -> Self {
        MessageQueue {
            queue: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn push(&mut self, msg: Message) {
        self.queue.lock().unwrap().push(msg)
    }

    pub fn flush(&mut self) -> Vec<Message> {
        if self.is_empty() {
            return Vec::new();
        }

        let mut queue = self.queue.lock().unwrap();
        std::mem::take(&mut queue)
    }

    pub fn len(&self) -> usize {
        self.queue.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.lock().unwrap().is_empty()
    }
}

impl Default for MessageQueue {
    fn default() -> Self {
        Self::new()
    }
}
