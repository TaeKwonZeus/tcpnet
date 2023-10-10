use std::{
    collections::HashMap,
    error::Error,
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
    task::JoinHandle,
};

use crate::common::{write_data, MessageQueue};

#[derive(Clone)]
pub struct ServerOpts {
    pub addr: String,
    pub on_connect: fn(SocketAddr),
    pub on_disconnect: fn(SocketAddr),
}

impl Default for ServerOpts {
    fn default() -> Self {
        Self {
            addr: "127.0.0.1:7000".to_owned(),
            on_connect: |_| {},
            on_disconnect: |_| {},
        }
    }
}

type Message = (SocketAddr, Vec<u8>);
type ServerQueue = MessageQueue<Message>;

pub struct Server {
    opts: ServerOpts,
    write_tx: mpsc::UnboundedSender<Message>,
    queue: ServerQueue,
    handle: JoinHandle<()>,
}

impl Server {
    pub fn new(opts: ServerOpts) -> Self {
        let (write_tx, write_rx) = mpsc::unbounded_channel();
        let queue = ServerQueue::new();

        let worker = ServerWorker {
            opts: opts.clone(),
            queue: queue.clone(),
        };
        let handle = tokio::spawn(async move { worker.run(write_rx).await });

        Self {
            opts,
            write_tx,
            queue,
            handle,
        }
    }

    pub fn received(&mut self) -> Vec<Message> {
        self.queue.flush()
    }

    pub fn send(&self, msg: Message) -> Result<(), Box<dyn Error>> {
        self.write_tx.send(msg)?;
        Ok(())
    }

    pub fn opts(&self) -> &ServerOpts {
        &self.opts
    }

    pub fn running(&self) -> bool {
        !self.handle.is_finished()
    }
}

struct ServerWorker {
    opts: ServerOpts,
    queue: ServerQueue,
}

impl ServerWorker {
    async fn run(&self, write_rx: mpsc::UnboundedReceiver<Message>) {
        let ln = TcpListener::bind(&self.opts.addr).await.unwrap();

        let writers: WritersMap = Arc::new(Mutex::new(HashMap::new()));
        let mut writer = WriterWorker {
            writers: writers.clone(),
            rx: write_rx,
        };
        tokio::spawn(async move { writer.run().await.unwrap() });

        eprintln!("Listening at address {}", self.opts.addr);

        while let Ok((conn, addr)) = ln.accept().await {
            let (read_half, write_half) = conn.into_split();

            let w = writers.clone();
            let mut listener = ListenerWorker {
                addr,
                reader: read_half,
                queue: self.queue.clone(),
            };
            let on_disconnect = self.opts.on_disconnect;

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
                on_disconnect(addr);
            });

            (self.opts.on_connect)(addr);
        }
    }
}

struct ListenerWorker {
    addr: SocketAddr,
    reader: OwnedReadHalf,
    queue: ServerQueue,
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
            let mut buf = vec![0u8; len as usize];
            let n = match self.reader.read_exact(&mut buf).await {
                Ok(n) => n,
                Err(e) if e.kind() == ErrorKind::UnexpectedEof => return Ok(()),
                Err(e) => return Err(e),
            };

            eprintln!("Received {} bytes from {}", n, self.addr);

            self.queue.push((self.addr, buf));
        }
    }
}

type WritersMap = Arc<Mutex<HashMap<SocketAddr, OwnedWriteHalf>>>;

struct WriterWorker {
    writers: WritersMap,
    rx: mpsc::UnboundedReceiver<Message>,
}

impl WriterWorker {
    async fn run(&mut self) -> io::Result<()> {
        while let Some((addr, mut data)) = self.rx.recv().await {
            if let Some(writer) = self.writers.lock().await.get_mut(&addr) {
                match write_data(writer, &mut data).await {
                    Ok(_) => eprintln!("Wrote {} bytes to {}", data.len(), addr),
                    Err(e) => eprintln!("Error while writing: {}", e),
                }
            }
        }

        Ok(())
    }
}
