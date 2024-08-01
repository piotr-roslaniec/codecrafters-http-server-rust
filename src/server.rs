use std::sync::Arc;

use bytes::Bytes;
use eyre::{Result, WrapErr};
use futures::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_util::codec::{BytesCodec, FramedRead, FramedWrite};

use crate::{
    error::{HttpError::EmptyRequestLine, ServerError},
    http::{HttpRequest, KEEP_ALIVE},
    router::Router,
};

/// A simple HTTP server.
#[derive(Clone)]
pub struct Server {
    addr:   String,
    router: Arc<Router>,
}

impl Server {
    /// Creates a new `Server` instance.
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to bind the server to.
    /// * `router` - The router to handle HTTP requests.
    ///
    /// # Returns
    ///
    /// A `Result` containing the `Server` instance or an error.
    pub(crate) fn new(addr: &str, router: Router) -> Result<Server> {
        Ok(Self { addr: addr.to_string(), router: Arc::new(router) })
    }

    /// Starts the server and listens for incoming connections.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or failure.
    pub(crate) async fn listen(&self) -> Result<()> {
        let listener = TcpListener::bind(&self.addr).await?;

        loop {
            let (stream, _) = listener.accept().await?;
            let router = self.router.clone();

            tokio::spawn(async move {
                if let Err(e) = Self::handle_connection(stream, router).await {
                    eprintln!("Connection error: {:?}", e);
                }
            });
        }
    }

    /// Handles an individual connection.
    ///
    /// # Arguments
    ///
    /// * `stream` - The TCP stream for the connection.
    /// * `router` - The router to handle HTTP requests.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or failure.
    async fn handle_connection(
        mut stream: tokio::net::TcpStream,
        router: Arc<Router>,
    ) -> Result<()> {
        let (reader, writer) = stream.split();
        let mut reader = FramedRead::new(reader, BytesCodec::new());
        let mut writer = FramedWrite::new(writer, BytesCodec::new());
        loop {
            let request_bytes = reader
                .next()
                .await
                .ok_or_else(|| ServerError::HttpError(EmptyRequestLine))
                .wrap_err("Failed to request read bytes")??;
            let request =
                HttpRequest::from_bytes(&request_bytes).wrap_err("Failed to parse request")?;

            let response = router.resolve(&request).wrap_err("Failed to resolve request")?;
            let response_bytes = response.to_bytes().wrap_err("Failed to serialize response")?;
            writer.send(Bytes::from(response_bytes)).await.wrap_err("Failed to send response")?;

            if request.connection != KEEP_ALIVE {
                break;
            }
        }
        Ok(())
    }
}
