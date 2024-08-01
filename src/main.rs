use eyre::Result;

use crate::{router::make_router, server::Server};

mod error;
mod http;
mod router;
mod server;

const DEFAULT_DIRECTORY: &str = "./public";

const DEFAULT_ADDR: &str = "127.0.0.1:4221";

#[tokio::main]
async fn main() -> Result<()> {
    let pub_dir = parse_cli_args();
    let router = make_router(&pub_dir);
    let server = Server::new(DEFAULT_ADDR, router)?;
    server.listen().await
}

fn parse_cli_args() -> String {
    let args = std::env::args().skip(1).collect::<Vec<String>>();
    if args.len() == 2 && args[0] == "--directory" {
        args[1].clone()
    } else if !args.is_empty() {
        println!("Usage: http-server --directory DIRECTORY");
        std::process::exit(1);
    } else {
        DEFAULT_DIRECTORY.to_string()
    }
}
