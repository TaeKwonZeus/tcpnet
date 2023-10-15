use std::{error::Error, fmt, io::ErrorKind};
use tokio::{
    io::AsyncReadExt,
    net::TcpStream,
    runtime::Runtime,
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

use crate::common::{write_data, MessageQueue};

/* -------------------------------------------------------------------------- */
/*                                  EXTERNAL                                  */
/* -------------------------------------------------------------------------- */

/// This error can indicate three things:
/// - `start()` hasn't been called on the client;
/// - `stop()` has been called;
/// - The server disconnected the client for any reason.
#[derive(Debug)]
pub struct NotConnectedError;

impl fmt::Display for NotConnectedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "attempting to call not started client")
    }
}

impl Error for NotConnectedError {}

/// Represents messages received from the client. Notifies the consumer about incoming data or disconnecting.
#[derive(Clone)]
pub enum Message {
    Disconnect,
    Data(Vec<u8>),
}

/// The client. Run `start()` to start the client, `stop()` to stop it.
pub struct Client {
    handle: Option<ClientHandle>,
    rt: Runtime,
}

impl Client {
    /// Create a new `Client`.
    pub fn new() -> Self {
        Self {
            handle: None,
            rt: Runtime::new().unwrap(),
        }
    }

    /// Start the client.
    pub fn start(&mut self, addr: &str) {
        let handle = self.rt.block_on(async { ClientHandle::new(addr) });
        self.handle = Some(handle);
    }

    /// Stop the client. The client can be restarted by calling `start()`.
    pub fn stop(&mut self) {
        self.handle = None;
    }

    /// Send bytes to the server.
    pub fn send(&self, data: Vec<u8>) -> Result<(), NotConnectedError> {
        if self.connected() {
            self.rt
                .block_on(async { self.handle.as_ref().unwrap().send(data) })?;
            Ok(())
        } else {
            Err(NotConnectedError)
        }
    }

    /// Gets the messages received from the server since the last `received()` call.
    pub fn received(&mut self) -> Result<Vec<Message>, NotConnectedError> {
        if self.connected() {
            self.rt
                .block_on(async { self.handle.as_mut().unwrap().received() })
        } else {
            Err(NotConnectedError)
        }
    }

    pub fn connected(&self) -> bool {
        match &self.handle {
            Some(h) => h.connected(),
            None => false,
        }
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

/* -------------------------------------------------------------------------- */
/*                                  INTERNAL                                  */
/* -------------------------------------------------------------------------- */

enum ClientMessage {
    Write(Vec<u8>),
    Stop,
}

struct ClientHandle {
    queue: MessageQueue<Message>,
    tx: mpsc::UnboundedSender<ClientMessage>,
    handle: JoinHandle<()>,
}

impl ClientHandle {
    fn new(addr: &str) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let queue = MessageQueue::new();

        let mut worker = ClientWorker {
            queue: queue.clone(),
            rx,
        };

        let a = addr.to_owned();
        let handle = tokio::spawn(async move { worker.run(a).await });

        Self { queue, tx, handle }
    }

    fn received(&mut self) -> Result<Vec<Message>, NotConnectedError> {
        if self.connected() {
            Ok(self.queue.flush())
        } else {
            Err(NotConnectedError)
        }
    }

    fn send(&self, data: Vec<u8>) -> Result<(), NotConnectedError> {
        if self.connected() {
            let _ = self.tx.send(ClientMessage::Write(data));
            Ok(())
        } else {
            Err(NotConnectedError)
        }
    }

    fn connected(&self) -> bool {
        !self.handle.is_finished()
    }
}

impl Drop for ClientHandle {
    fn drop(&mut self) {
        let _ = self.tx.send(ClientMessage::Stop);
    }
}

struct ClientWorker {
    queue: MessageQueue<Message>,
    rx: mpsc::UnboundedReceiver<ClientMessage>,
}

impl ClientWorker {
    async fn run(&mut self, addr: String) {
        let conn = TcpStream::connect(addr).await.unwrap();
        let (mut read_half, mut write_half) = conn.into_split();
        println!("Connected to server");

        // Start listener, too simple for an actor
        let mut q = self.queue.clone();
        let (stop_tx, mut stop_rx) = oneshot::channel();
        tokio::spawn(async move {
            loop {
                // Get length of message
                let mut len_buf = [0u8; 4];
                match read_half.read_exact(len_buf.as_mut_slice()).await {
                    Ok(_) => {}
                    Err(e) if e.kind() == ErrorKind::UnexpectedEof => break,
                    Err(e) => {
                        eprintln!("Error while reading: {}", e);
                        break;
                    }
                }
                let len = u32::from_le_bytes(len_buf);

                // Get message with the length len
                let mut buf = vec![0u8; len as usize];
                let n = match read_half.read_exact(&mut buf).await {
                    Ok(n) => n,
                    Err(e) if e.kind() == ErrorKind::UnexpectedEof => break,
                    Err(e) => {
                        eprintln!("Error while reading: {}", e);
                        break;
                    }
                };

                println!("Received {} bytes from server", n);

                q.push(Message::Data(buf));
            }
            let _ = stop_tx.send(());
        });

        loop {
            let _ = tokio::select! {
                // Wait for handle completion
                _ = &mut stop_rx => {
                    self.queue.push(Message::Disconnect);
                    println!("Disconnected from server");
                    return;
                },
                Some(msg) = self.rx.recv() => {
                    match msg {
                        ClientMessage::Write(mut data) => write_data(&mut write_half, &mut data).await,
                        ClientMessage::Stop => return
                    }
                }
            };
        }
    }
}
