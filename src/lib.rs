mod common;
mod server;

pub use server::{Server, ServerOpts};

#[cfg(test)]
pub mod tests {
    use crate::*;

    #[tokio::test]
    async fn test_server() {
        let server = Server::new(ServerOpts::default());

        assert!(server.running());
    }
}
