use std::{net::SocketAddr, sync::Arc};

use bytes::Bytes;
use tokio::sync::Mutex;

pub struct Message {
    pub addr: SocketAddr,
    pub data: Bytes,
}

#[derive(Clone)]
pub struct MessageQueue {
    queue: Arc<Mutex<Vec<Message>>>,
}

impl MessageQueue {
    pub fn new() -> Self {
        MessageQueue {
            queue: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn push(&mut self, msg: Message) {
        self.queue.lock().await.push(msg)
    }

    pub async fn flush(&mut self) -> Vec<Message> {
        let mut queue = self.queue.lock().await;
        std::mem::take(&mut queue)
    }

    pub async fn len(&self) -> usize {
        self.queue.lock().await.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.queue.lock().await.is_empty()
    }
}

impl Default for MessageQueue {
    fn default() -> Self {
        Self::new()
    }
}
