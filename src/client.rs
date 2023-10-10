use std::{error::Error, io::ErrorKind};

use tokio::{
    io::{self, AsyncReadExt},
    net::{tcp::OwnedWriteHalf, TcpStream},
    sync::mpsc,
    task::JoinHandle,
};

use crate::common::{write_data, MessageQueue};

#[derive(Clone)]
pub struct ClientOpts {
    pub addr: String,
    pub on_connect: fn(),
    pub on_disconnect: fn(),
}

impl Default for ClientOpts {
    fn default() -> Self {
        Self {
            addr: "127.0.0.1:7000".to_owned(),
            on_connect: || {},
            on_disconnect: || {},
        }
    }
}

type Message = Vec<u8>;
type ClientQueue = MessageQueue<Message>;

pub struct Client {
    opts: ClientOpts,
    queue: ClientQueue,
    write_tx: mpsc::UnboundedSender<Message>,
    handle: JoinHandle<()>,
}

impl Client {
    pub fn new(opts: ClientOpts) -> Self {
        let (write_tx, write_rx) = mpsc::unbounded_channel();
        let queue = ClientQueue::new();

        let mut worker = ClientWorker {
            opts: opts.clone(),
            queue: queue.clone(),
        };
        let on_disconnect = opts.on_disconnect;
        let handle = tokio::spawn(async move {
            worker.run(write_rx).await.unwrap();
            on_disconnect();
        });

        Self {
            opts,
            queue,
            write_tx,
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

    pub fn opts(&self) -> &ClientOpts {
        &self.opts
    }

    pub fn connected(&self) -> bool {
        !self.handle.is_finished()
    }
}

struct ClientWorker {
    opts: ClientOpts,
    queue: ClientQueue,
}

impl ClientWorker {
    async fn run(&mut self, write_rx: mpsc::UnboundedReceiver<Message>) -> io::Result<()> {
        let (mut read_half, write_half) = TcpStream::connect(&self.opts.addr).await?.into_split();
        (self.opts.on_connect)();

        let mut writer = WriterWorker {
            writer: write_half,
            rx: write_rx,
        };
        tokio::spawn(async move { writer.run().await });

        loop {
            // Get length of message
            let mut len_buf = [0u8; 4];
            match read_half.read_exact(len_buf.as_mut_slice()).await {
                Ok(_) => {}
                Err(e) if e.kind() == ErrorKind::UnexpectedEof => return Ok(()),
                Err(e) => return Err(e),
            }
            let len = u32::from_le_bytes(len_buf);

            // Get message with the length len
            let mut buf = vec![0u8; len as usize];
            let n = match read_half.read_exact(&mut buf).await {
                Ok(n) => n,
                Err(e) if e.kind() == ErrorKind::UnexpectedEof => return Ok(()),
                Err(e) => return Err(e),
            };

            eprintln!("Received {} bytes from server", n);

            self.queue.push(buf);
        }
    }
}

struct WriterWorker {
    writer: OwnedWriteHalf,
    rx: mpsc::UnboundedReceiver<Message>,
}

impl WriterWorker {
    async fn run(&mut self) -> io::Result<()> {
        while let Some(mut msg) = self.rx.recv().await {
            match write_data(&mut self.writer, &mut msg).await {
                Ok(_) => eprintln!("Wrote {} bytes to server", msg.len()),
                Err(e) => eprintln!("Error while writing: {}", e),
            }
        }

        Ok(())
    }
}
