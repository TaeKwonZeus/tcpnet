use bytes::BytesMut;
use std::{
    collections::HashMap,
    io::{self, ErrorKind},
    net::SocketAddr,
    sync::Arc,
};
use tokio::{
    io::AsyncReadExt,
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpListener,
    },
    sync::{mpsc, Mutex},
};

use crate::common::{Message, MessageQueue};

#[derive(Clone)]
pub struct ServerOpts {
    pub addr: &'static str,
    pub on_connect: fn(SocketAddr),
    pub on_disconnect: fn(SocketAddr),
}

impl Default for ServerOpts {
    fn default() -> Self {
        Self {
            addr: "127.0.0.1:7000",
            on_connect: |_| {},
            on_disconnect: |_| {},
        }
    }
}

pub struct Server {
    opts: ServerOpts,
    write_tx: mpsc::UnboundedSender<Message>,
    queue: MessageQueue,
}

impl Server {
    pub fn new(opts: ServerOpts) -> Self {
        let (write_tx, write_rx) = mpsc::unbounded_channel();
        let queue = MessageQueue::new();

        let worker = ServerWorker {
            opts: opts.clone(),
            write_rx,
            queue: queue.clone(),
        };
        tokio::spawn(async move { worker.run().await });

        Self {
            opts,
            write_tx,
            queue,
        }
    }

    pub async fn received(&mut self) -> Vec<Message> {
        self.queue.flush().await
    }
}

struct ServerWorker {
    opts: ServerOpts,
    write_rx: mpsc::UnboundedReceiver<Message>,
    queue: MessageQueue,
}

impl ServerWorker {
    async fn run(&self) {
        let ln = TcpListener::bind(self.opts.addr).await.unwrap();

        let writers: Arc<Mutex<HashMap<SocketAddr, OwnedWriteHalf>>> =
            Arc::new(Mutex::new(HashMap::new()));

        while let Ok((conn, addr)) = ln.accept().await {
            let (read_half, write_half) = conn.into_split();

            let w = writers.clone();
            let mut listener = ListenerWorker {
                addr,
                reader: read_half,
                queue: self.queue.clone(),
            };
            tokio::spawn(async move {
                // Nest scope so mutex gets unlocked before running listener
                {
                    w.lock().await.insert(addr, write_half);
                }
                eprintln!("Client at address {} connected", addr);

                if let Err(e) = listener.run().await {
                    eprintln!("Error encountered while reading: {}", e);
                };

                w.lock().await.remove(&addr);
                eprintln!("Client at address {} disconnected", addr);
            });
        }
    }
}

struct ListenerWorker {
    addr: SocketAddr,
    reader: OwnedReadHalf,
    queue: MessageQueue,
}

impl ListenerWorker {
    async fn run(&mut self) -> io::Result<()> {
        loop {
            // Get length of message
            let mut len_buf = [0u8; 4];
            match self.reader.read_exact(len_buf.as_mut_slice()).await {
                Ok(_) => {}
                Err(e) if e.kind() == ErrorKind::UnexpectedEof => return Ok(()),
                Err(e) => return Err(e),
            }
            let len = u32::from_le_bytes(len_buf);

            // Get message with the length len
            let mut buf = BytesMut::with_capacity(len as usize);
            while buf.len() < buf.capacity() {
                if self.reader.read_buf(&mut buf).await? == 0 {
                    return Ok(());
                };
            }

            self.queue
                .push(Message {
                    addr: self.addr,
                    data: buf.freeze(),
                })
                .await;
        }
    }
}
