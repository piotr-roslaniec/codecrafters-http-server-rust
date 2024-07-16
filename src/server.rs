use crate::http::HttpRequest;
use crate::router::Router;
use eyre::Result;
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_util::codec::{FramedRead, FramedWrite, LinesCodec};

#[derive(Clone)]
pub struct Server {
    addr: String,
    router: Arc<Router>,
}

impl Server {
    pub(crate) fn new(addr: &str, router: Router) -> Result<Server> {
        Ok(Self {
            addr: addr.to_string(),
            router: Arc::new(router),
        })
    }

    pub(crate) async fn listen(&self) -> Result<()> {
        let listener = TcpListener::bind(&self.addr).await?;

        loop {
            let (mut stream, _) = listener.accept().await?;
            let router = self.router.clone();

            tokio::spawn(async move {
                let (reader, writer) = stream.split();
                let mut reader = FramedRead::new(reader, LinesCodec::new());
                let mut writer = FramedWrite::new(writer, LinesCodec::new());

                loop {
                    let mut lines = Vec::new();
                    while let Some(Ok(msg)) = reader.next().await {
                        if msg.is_empty() {
                            break;
                        }
                        lines.push(msg);
                    }

                    if lines.is_empty() {
                        break;
                    }

                    let request = match HttpRequest::from_lines(&lines) {
                        Ok(req) => req,
                        Err(e) => {
                            eprintln!("Failed to parse request: {:?}", e);
                            break;
                        }
                    };

                    let response = match router.resolve(&request) {
                        Ok(resp) => resp,
                        Err(e) => {
                            eprintln!("Failed to resolve request: {:?}", e);
                            break;
                        }
                    };

                    if let Err(e) = writer.send(response.to_string().unwrap_or_default()).await {
                        eprintln!("Failed to send response: {:?}", e);
                        break;
                    }

                    if request.connection != *"keep-alive" {
                        break;
                    }
                }
            });
        }
    }
}
