use crate::router::make_router;
use crate::server::Server;
use eyre::Result;

mod error;
mod http;
mod router;
mod server;

#[tokio::main]
async fn main() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<String>>();
    let directory = if args.len() == 2 && args[0] == "--directory" {
        Some(args[1].clone())
    } else if !args.is_empty() {
        println!("Usage: http-server --directory DIRECTORY");
        std::process::exit(1);
    } else {
        None
    };
    let router = make_router(directory);
    let server = Server::new("127.0.0.1:4221", router)?;
    server.listen().await
}
