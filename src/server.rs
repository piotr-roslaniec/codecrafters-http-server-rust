use crate::error::Result;
use crate::http::HttpRequest;
use crate::router::Router;
use std::net::TcpListener;

pub struct Server {
    listener: TcpListener,
    router: Router,
}

impl Server {
    pub(crate) fn new(addr: &str, router: Router) -> Result<Server> {
        let listener = TcpListener::bind(addr)?;
        Ok(Self { listener, router })
    }

    pub(crate) fn listen(&self) -> Result<()> {
        for stream in self.listener.incoming() {
            let mut accepted_stream = stream?;
            let request = HttpRequest::from_tcp_stream(&mut accepted_stream)?;
            let response = self.router.resolve(request)?;
            response.write_to_stream(&mut accepted_stream)?;
        }
        Ok(())
    }
}
