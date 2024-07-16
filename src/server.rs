use crate::http::HttpRequest;
use crate::router::Router;
use eyre::Result;
use futures::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_util::codec::{FramedRead, FramedWrite, LinesCodec};

pub struct Server {
    addr: String,
    router: Router,
}

impl Server {
    pub(crate) fn new(addr: &str, router: Router) -> Result<Server> {
        Ok(Self {
            addr: addr.to_string(),
            router,
        })
    }

    pub(crate) async fn listen(&self) -> Result<()> {
        let listener = TcpListener::bind(&self.addr).await?;

        loop {
            // Accept a new connection
            let (mut stream, _) = listener.accept().await?;
            let (reader, writer) = stream.split();

            let mut reader = FramedRead::new(reader, LinesCodec::new());
            let mut writer = FramedWrite::new(writer, LinesCodec::new());

            // Handle the connection
            loop {
                let mut lines = Vec::new();
                while let Some(Ok(msg)) = reader.next().await {
                    if msg.is_empty() {
                        break;
                    }
                    lines.push(msg);
                }

                // Break connection if no lines were read
                if lines.is_empty() {
                    break;
                }

                let request = HttpRequest::from_lines(&lines)?;
                let response = self.router.resolve(&request)?;

                writer.send(response.to_string()?).await?;

                // Break connection if the connection header is not set to keep-alive
                if request.connection.as_deref() != Some("keep-alive") {
                    break;
                }
            }
        }
    }
}
