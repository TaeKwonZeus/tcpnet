mod client;
mod common;
mod server;

pub use client::{Client, ClientOpts};
pub use server::Server;

#[cfg(test)]
pub mod tests {
    use std::time::Duration;

    use crate::*;

    #[test]
    fn test() {
        let mut server = Server::new();
        server.start(7000);
        assert!(server.running());

        // let mut client = Client::new(ClientOpts::default());
        // assert!(client.connected());

        // client.send("Hello!".as_bytes().to_vec()).unwrap();

        // // Wait 1 millisecond for TCP to transfer data
        // std::thread::sleep(Duration::from_millis(1));

        // let recv = server.received().unwrap();
        // assert_eq!(recv.len(), 1);
        // assert_eq!(recv[0].1, "Hello!".as_bytes().to_vec());

        // server
        //     .send((recv[0].0, "Hello back!".as_bytes().to_vec()))
        //     .unwrap();

        // // Wait 1 millisecond for TCP to transfer data
        // std::thread::sleep(Duration::from_millis(1));

        // let recv = client.received();
        // assert_eq!(recv.len(), 1);
        // assert_eq!(recv[0], "Hello back!".as_bytes().to_vec());
    }
}
