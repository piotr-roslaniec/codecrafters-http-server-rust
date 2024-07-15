use crate::error::Result;
use crate::router::make_router;
use crate::server::Server;

mod error;
mod http;
mod router;
mod server;

fn main() -> Result<()> {
    let router = make_router();
    let server = Server::new("127.0.0.1:4221", router)?;
    server.listen()?;
    Ok(())
}
