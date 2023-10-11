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
