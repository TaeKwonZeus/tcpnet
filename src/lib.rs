pub mod client;
mod common;
pub mod server;

#[cfg(test)]
pub mod tests {
    use std::{error::Error, time::Duration};

    use crate::{
        client::Client,
        server::{Message, Server},
    };

    #[test]
    fn test() -> Result<(), Box<dyn Error>> {
        let mut server = Server::new();
        server.start(7000);
        assert!(server.running());

        let mut client = Client::new();
        client.start("127.0.0.1:7000");
        assert!(client.connected());

        client.send("Hello!".as_bytes().to_vec())?;
        client.stop();

        std::thread::sleep(Duration::from_millis(1));

        // Restart the client
        client.start("127.0.0.1:7000");

        // Wait for TCP
        std::thread::sleep(Duration::from_millis(1));

        // Process messages
        for msg in server.received()? {
            match msg {
                Message::Connect(addr) => println!("{} connected", addr),
                Message::Disconnect(addr) => println!("{} disconnected", addr),
                Message::Data(addr, data) => println!("{}: {}", addr, std::str::from_utf8(&data)?),
            }
        }

        Ok(())
    }
}
