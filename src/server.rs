use std::{
    collections::HashMap,
    error::Error,
    fmt,
    io::{self, ErrorKind},
    net::SocketAddr,
};
use tokio::{
    io::AsyncReadExt,
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpListener, TcpStream,
    },
    runtime::Runtime,
    sync::mpsc,
    task::JoinHandle,
};

use crate::common::{write_data, MessageQueue};

#[derive(Debug)]
pub struct ServerNotStartedError();

impl fmt::Display for ServerNotStartedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "attempting to call not started server")
    }
}

impl Error for ServerNotStartedError {}

#[derive(Clone)]
pub enum Message {
    Connect(SocketAddr),
    Disconnect(SocketAddr),
    Data(SocketAddr, Vec<u8>),
}

pub struct Server {
    handle: Option<ServerHandle>,
    rt: Runtime,
}

#[allow(clippy::new_without_default)]
impl Server {
    pub fn new() -> Self {
        Self {
            handle: None,
            rt: Runtime::new().unwrap(),
        }
    }

    pub fn start(&mut self, port: u16) {
        let handle = self.rt.block_on(async { ServerHandle::new(port) });

        self.handle = Some(handle);
    }

    pub fn stop(&mut self) {
        if !self.running() {
            return;
        }

        self.rt.block_on(async { self.handle = None });
    }

    pub fn disconnect(&mut self, addr: SocketAddr) -> Result<(), Box<dyn Error>> {
        if self.running() {
            self.rt
                .block_on(async { self.handle.as_ref().unwrap().disconnect(addr) })?;
            Ok(())
        } else {
            Err(Box::new(ServerNotStartedError()))
        }
    }

    pub fn received(&mut self) -> Result<Vec<Message>, ServerNotStartedError> {
        if self.running() {
            self.rt
                .block_on(async { self.handle.as_mut().unwrap().received() })
        } else {
            Err(ServerNotStartedError())
        }
    }

    pub fn send(&mut self, addr: SocketAddr, data: Vec<u8>) -> Result<(), Box<dyn Error>> {
        if self.running() {
            self.rt
                .block_on(async { self.handle.as_ref().unwrap().send(addr, data) })?;
            Ok(())
        } else {
            Err(Box::new(ServerNotStartedError()))
        }
    }

    pub fn running(&mut self) -> bool {
        match self.handle.as_ref() {
            Some(h) => {
                if self.rt.block_on(async { h.running() }) {
                    true
                } else {
                    self.handle = None;
                    false
                }
            }
            None => false,
        }
    }
}

struct ServerHandle {
    tx: mpsc::UnboundedSender<ServerMessage>,
    queue: MessageQueue,
    handle: JoinHandle<()>,
}

impl ServerHandle {
    fn new(port: u16) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let queue = MessageQueue::new();

        let mut worker = ServerWorker {
            rx,
            port,
            queue: queue.clone(),
        };
        let handle = tokio::spawn(async move { worker.run().await });

        Self { tx, queue, handle }
    }

    fn received(&mut self) -> Result<Vec<Message>, ServerNotStartedError> {
        if self.running() {
            Ok(self.queue.flush())
        } else {
            Err(ServerNotStartedError())
        }
    }

    fn send(&self, addr: SocketAddr, data: Vec<u8>) -> Result<(), Box<dyn Error>> {
        if self.running() {
            self.tx.send(ServerMessage::Write(addr, data))?;
            Ok(())
        } else {
            Err(Box::new(ServerNotStartedError()))
        }
    }

    fn disconnect(&self, addr: SocketAddr) -> Result<(), Box<dyn Error>> {
        if self.running() {
            self.tx.send(ServerMessage::Disconnect(addr))?;
            Ok(())
        } else {
            Err(Box::new(ServerNotStartedError()))
        }
    }

    fn running(&self) -> bool {
        !self.handle.is_finished()
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        self.tx.send(ServerMessage::Stop);
        self.handle.abort();
    }
}

enum ServerMessage {
    Stop,
    Disconnect(SocketAddr),
    Write(SocketAddr, Vec<u8>),
}

struct ServerWorker {
    rx: mpsc::UnboundedReceiver<ServerMessage>,
    port: u16,
    queue: MessageQueue,
}

impl ServerWorker {
    async fn run(&mut self) {
        let ln = TcpListener::bind(format!("127.0.0.1:{}", self.port))
            .await
            .unwrap();

        let writer = Writer::new();
        let mut listeners = HashMap::<SocketAddr, Listener>::new();

        println!("Listening on port {}", self.port);

        loop {
            tokio::select! {
                res = ln.accept() => {
                    match res {
                        Ok((conn, addr)) => {
                            listeners.insert(addr, Listener::new(addr, conn, self.queue.clone(), writer.clone()));
                        },
                        Err(e) => {
                            eprintln!("Error encountered while accepting connection: {}", e);
                            return;
                        }
                    }
                }
                Some(msg) = self.rx.recv() => {
                    match msg {
                        ServerMessage::Stop => {
                            let _ = writer.send(WriterMessage::Stop);
                            eprintln!("Stopping server");
                            return;
                        },
                        ServerMessage::Disconnect(addr) => {
                            listeners.remove(&addr);
                            let _ = writer.send(WriterMessage::RemoveWriter(addr));
                        },
                        ServerMessage::Write(addr, data) => {
                            let _ = writer.send(WriterMessage::Write(addr, data));
                        },
                    };
                }
            }
        }
    }
}

struct Listener {
    addr: SocketAddr,
    handle: JoinHandle<()>,
    writer: Writer,
}

impl Listener {
    fn new(addr: SocketAddr, conn: TcpStream, queue: MessageQueue, writer: Writer) -> Self {
        let (read_half, write_half) = conn.into_split();
        let mut worker = ListenerWorker {
            addr,
            reader: read_half,
            queue: queue.clone(),
        };
        let w = writer.clone();
        let handle = tokio::spawn(async move {
            let _ = w.send(WriterMessage::AddWriter(addr, write_half));
            println!("Client at address {} connected", addr);
            worker.queue.push(Message::Connect(addr));

            worker.run().await.unwrap();

            let _ = w.send(WriterMessage::RemoveWriter(addr));
            println!("Client at address {} disconnected", addr);
            worker.queue.push(Message::Disconnect(addr));
        });

        Self {
            addr,
            handle,
            writer,
        }
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        let _ = self.writer.send(WriterMessage::RemoveWriter(self.addr));
        self.handle.abort();
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
            let mut buf = vec![0u8; len as usize];
            let n = match self.reader.read_exact(&mut buf).await {
                Ok(n) => n,
                Err(e) if e.kind() == ErrorKind::UnexpectedEof => return Ok(()),
                Err(e) => return Err(e),
            };

            eprintln!("Received {} bytes from {}", n, self.addr);

            self.queue.push(Message::Data(self.addr, buf));
        }
    }
}

enum WriterMessage {
    Stop,
    Write(SocketAddr, Vec<u8>),
    AddWriter(SocketAddr, OwnedWriteHalf),
    RemoveWriter(SocketAddr),
}

#[derive(Clone)]
struct Writer {
    tx: mpsc::UnboundedSender<WriterMessage>,
}

impl Writer {
    fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut worker = WriterWorker { rx };
        tokio::spawn(async move { worker.run().await });

        Self { tx }
    }

    fn write(&self, addr: SocketAddr, data: Vec<u8>) {
        self.tx.send(WriterMessage::Write(addr, data));
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        self.tx.send(WriterMessage::Stop);
    }
}

struct WriterWorker {
    rx: mpsc::UnboundedReceiver<WriterMessage>,
}

impl WriterWorker {
    async fn run(&mut self) -> io::Result<()> {
        let mut writers = HashMap::<SocketAddr, OwnedWriteHalf>::new();

        while let Some(msg) = self.rx.recv().await {
            match msg {
                WriterMessage::Stop => return Ok(()),
                WriterMessage::Write(addr, data) => Self::write(&mut writers, addr, data).await,
                WriterMessage::AddWriter(addr, writer) => {
                    writers.insert(addr, writer);
                }
                WriterMessage::RemoveWriter(addr) => {
                    writers.remove(&addr);
                }
            };
        }

        Ok(())
    }

    async fn write(
        writers: &mut HashMap<SocketAddr, OwnedWriteHalf>,
        addr: SocketAddr,
        mut data: Vec<u8>,
    ) {
        if let Some(writer) = writers.get_mut(&addr) {
            match write_data(writer, &mut data).await {
                Ok(_) => {
                    println!("Wrote {} bytes to {}", data.len(), addr);
                }
                Err(e) => {
                    eprintln!("Error while writing data: {}", e);
                }
            }
        }
    }
}
