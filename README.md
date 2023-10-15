# tcpnet

A simple tick-based TCP message transport for games that runs on Tokio. The client and server provide `send()` and `receive()` methods to send data and poll events respectively.

Server usage example:

```rs
use tcpnet::server::{Event, Server};

fn main() {
    let mut server = Server::new();
    server.start(7000);
    assert!(server.running());

    // Process events on every tick
    for event in server.received().unwrap() {
        match event {
            Event::Connect(addr) => println!("{} connected", addr),
            Event::Disconnect(addr) => println!("{} disconnected", addr),
            Event::Data(addr, data) => {
                println!("{}: {}", addr, std::str::from_utf8(&data).unwrap());
                server.send(addr, "Hello back!".as_bytes().to_vec())?;
            }
        }
    }
}

```

Client usage example:

```rs
use tcpnet::client::{Event, Client};

fn main() {
    let mut client = Client::new();
    client.start("127.0.0.1:7000");
    assert!(server.running());

    // Process events on every tick/frame
    for event in server.received().unwrap() {
        match event {
            Event::Disconnect => println!("Disconnected from server"),
            Event::Data(data) => println!("Server: {}", addr, std::str::from_utf8(&data).unwrap()),
        }
    }
}
```
