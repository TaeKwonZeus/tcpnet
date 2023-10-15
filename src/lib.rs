pub mod client;
mod common;
pub mod server;

#[cfg(test)]
pub mod tests {
    use crate::{
        client::{self, Client},
        server::{self, Server},
    };
    use std::{error::Error, thread::sleep, time::Duration};

    #[test]
    fn test() -> Result<(), Box<dyn Error>> {
        let mut server = Server::new();
        server.start(7000);
        assert!(server.running());

        let mut client = Client::new();
        client.start("127.0.0.1:7000");
        assert!(client.connected());

        // Send data to server
        client.send("Hello!".as_bytes().to_vec())?;

        // Wait for TCP
        sleep(Duration::from_millis(1));

        // Process events
        for event in server.received()? {
            match event {
                server::Event::Connect(addr) => println!("{} connected", addr),
                server::Event::Disconnect(addr) => println!("{} disconnected", addr),
                server::Event::Data(addr, data) => {
                    println!("{}: {}", addr, std::str::from_utf8(&data)?);
                    server.send(addr, "Hello back!".as_bytes().to_vec())?;
                }
            }
        }

        sleep(Duration::from_millis(1));

        for event in client.received()? {
            if let client::Event::Data(data) = event {
                println!("Received data from server: {}", std::str::from_utf8(&data)?);
            }
        }

        Ok(())
    }
}
