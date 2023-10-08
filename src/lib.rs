pub mod common;
pub mod server;

#[cfg(test)]
mod tests {
    use super::server::*;

    #[test]
    fn test_server() {
        let _ = Server::new(ServerOpts {
            addr: "127.0.0.1:3000",
            ..Default::default()
        });
    }
}
