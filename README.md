# tcpnet

A simple tick-based TCP message transport for games that runs on Tokio. tcpnet leaves you the ability to process messages and send data to clients at any time with `Server.receive()` and `Server.send()`. The server only uses 1 tokio task per connection, with two more for writing responses and managing the server.

Server usage example:

```rs
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
        println!("{}, {:#?}", addr, data);

        // Sending messages to clients
        server.send((addr, "Hello!".as_bytes().to_vec())).unwrap();
    }
}

```

Client usage example:

```rs
use tcpnet::{Client, ClientOpts};

fn on_connect() {
    // Executed when connecting to server
    println!("Connected to server");
}

fn on_disconnect() {
    // Executed when disconnected from server
    println!("Disconnected from server");
}

#[tokio::main]
async fn main() {
    let opts = ClientOpts {
        addr: "127.0.0.1:7000".to_owned(),
        on_connect,
        on_disconnect,
    };

    // Creates a client and connects to server
    let mut client = Client::new(opts);

    // Send messages to server
    client.send("Hello!".as_bytes().to_vec()).unwrap();

    // Run this in your update loop
    // Receive messages from server
    for msg in client.received() {
        // Process messages
        println!("Server: {}", std::str::from_utf8(msg.as_slice()).unwrap());
    }
}
```
