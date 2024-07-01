use crate::error::Result;
use crate::server::Server;

mod error;
mod http;
mod server;

fn main() -> Result<()> {
    let server = Server::new("127.0.0.1:4221")?;
    server.listen()?;
    Ok(())
}
