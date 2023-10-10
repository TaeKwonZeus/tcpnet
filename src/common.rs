use std::sync::{Arc, Mutex};
use tokio::io;
use tokio::io::AsyncWriteExt;
use tokio::net::tcp::OwnedWriteHalf;

#[derive(Clone)]
pub struct MessageQueue<T> {
    queue: Arc<Mutex<Vec<T>>>,
}

#[allow(dead_code)]
impl<T> MessageQueue<T> {
    pub fn new() -> Self {
        MessageQueue {
            queue: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn push(&mut self, msg: T) {
        self.queue.lock().unwrap().push(msg)
    }

    pub fn flush(&mut self) -> Vec<T> {
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

impl<T> Default for MessageQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn write_data(writer: &mut OwnedWriteHalf, buf: &mut Vec<u8>) -> io::Result<()> {
    writer
        .write_all(&u32::to_le_bytes(buf.len() as u32))
        .await?;

    writer.write_all(buf).await
}
