use crate::error::Result;
use crate::http::Header;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

pub struct Server {
    listener: TcpListener,
}

impl Server {
    pub(crate) fn new(addr: &str) -> Result<Server> {
        let listener = TcpListener::bind(addr)?;
        Ok(Self { listener })
    }

    pub(crate) fn listen(&self) -> Result<()> {
        for stream in self.listener.incoming() {
            let mut accepted_stream = stream?;
            let mut request_buff = BufReader::new(&accepted_stream);

            let mut first_line = String::new();
            request_buff.read_line(&mut first_line)?;
            let header = Header::from_line(&first_line)?;

            let buffer = if header.path == *"/" {
                "HTTP/1.1 200 OK\r\n\r\n"
            } else {
                "HTTP/1.1 404 Not Found\r\n\r\n"
            };
            accepted_stream.write_all(buffer.as_ref())?;
        }
        Ok(())
    }
}
