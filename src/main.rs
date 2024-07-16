use crate::router::make_router;
use crate::server::Server;
use eyre::Result;

mod error;
mod http;
mod router;
mod server;

async fn run() -> Result<()> {
    let router = make_router();
    let server = Server::new("127.0.0.1:4221", router)?;
    server.listen().await
}

#[tokio::main]
async fn main() -> Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async {
            run().await?;
            Ok(())
        })
}
