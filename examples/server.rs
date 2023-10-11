use std::net::SocketAddr;
use tcpnet::{Server, ServerOpts};

fn on_connect(addr: SocketAddr) {
    // Handle connections
    println!("Client at address {} connected", addr);
}

fn on_disconnect(addr: SocketAddr) {
    // Handle clients disconnecting
    println!("Client at address {} disconnected", addr);
}

#[tokio::main]
async fn main() {
    let opts = ServerOpts {
        addr: "127.0.0.1:7000".to_owned(),
        on_connect,
        on_disconnect,
    };

    // Creates and starts the server in a background task
    let mut server = Server::new(opts);
    assert!(server.running());

    // Call this on every tick
    for (addr, data) in server.received() {
        // Handle incoming requests
        println!(
            "{}: {}",
            addr,
            std::str::from_utf8(data.as_slice()).unwrap()
        );

        // Sending messages to clients
        server.send((addr, "Hello!".as_bytes().to_vec())).unwrap();
    }
}
