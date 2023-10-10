mod client;
mod common;
mod server;

pub use client::{Client, ClientOpts};
pub use server::{Server, ServerOpts};

#[cfg(test)]
pub mod tests {
    use std::time::Duration;

    use crate::*;

    #[tokio::test]
    async fn test_server() {
        let mut server = Server::new(ServerOpts::default());
        assert!(server.running());

        let mut ticker = tokio::time::interval(Duration::from_millis(64));
        loop {
            ticker.tick().await;

            for (addr, msg) in server.received() {
                println!("{}: {}", addr, std::str::from_utf8(&msg).unwrap());
            }
        }
    }

    #[tokio::test]
    async fn test_client() {
        let client = Client::new(ClientOpts::default());
        assert!(client.connected());

        client
            .send("I hate this place".as_bytes().to_vec())
            .unwrap();

        let mut ticker = tokio::time::interval(Duration::from_secs(5));
        loop {
            ticker.tick().await;

            assert!(client.send("I hate this place".as_bytes().to_vec()).is_ok());
        }
    }
}
