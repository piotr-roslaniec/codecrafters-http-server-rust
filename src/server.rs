use crate::http::HttpRequest;
use crate::router::Router;
use bytes::Bytes;
use eyre::Result;
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_util::codec::{BytesCodec, FramedRead, FramedWrite};

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
                let mut reader = FramedRead::new(reader, BytesCodec::new());
                let mut writer = FramedWrite::new(writer, BytesCodec::new());

                loop {
                    let bytes = match reader.next().await {
                        Some(Ok(l)) => l,
                        Some(Err(e)) => {
                            eprintln!("Failed to read bytes: {:?}", e);
                            break;
                        }
                        None => break,
                    };
                    println!("Received bytes: {:?}", bytes);

                    let request = match HttpRequest::from_bytes(&bytes) {
                        Ok(req) => req,
                        Err(e) => {
                            eprintln!("Failed to parse request: {:?}", e);
                            break;
                        }
                    };
                    println!("Parsed request: {:?}", request);

                    let response = match router.resolve(&request) {
                        Ok(resp) => resp,
                        Err(e) => {
                            eprintln!("Failed to resolve request: {:?}", e);
                            break;
                        }
                    };
                    println!("Resolved response: {:?}", response);

                    if let Err(e) = writer.send(Bytes::from(response.to_bytes())).await {
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
