use crate::error::{HttpError, Result, ServerError};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;

const CRLF: &str = "\r\n";

#[derive(Debug, PartialEq)]
pub struct RequestLine {
    pub method: String,
    pub path: String,
    pub version: String,
}

impl RequestLine {
    pub fn new(method: &str, path: &str, version: &str) -> Self {
        Self {
            method: method.to_string(),
            path: path.to_string(),
            version: version.to_string(),
        }
    }

    pub fn from_line(line: &str) -> Result<Self> {
        let mut iter = line.split_whitespace();
        let method = iter
            .next()
            .ok_or(ServerError::HttpError(HttpError::MissingMethod))?
            .to_string();
        let path = iter
            .next()
            .ok_or(ServerError::HttpError(HttpError::MissingPath))?
            .to_string();
        let version = iter
            .next()
            .ok_or(ServerError::HttpError(HttpError::MissingVersion))?
            .to_string();
        Ok(Self {
            method,
            path,
            version,
        })
    }
}

type RequestHeaders = HashMap<String, String>;

#[derive(Debug)]
pub struct HttpRequest {
    pub request_line: RequestLine,
    pub request_headers: RequestHeaders,
}

impl HttpRequest {
    fn new(header: RequestLine, headers: RequestHeaders) -> Self {
        Self {
            request_line: header,
            request_headers: headers,
        }
    }

    pub fn from_tcp_stream(stream: &mut TcpStream) -> Result<HttpRequest> {
        let mut request_buff = BufReader::new(stream);

        let mut lines = Vec::new();
        loop {
            let mut line = String::new();
            request_buff.read_line(&mut line)?;
            if line == CRLF {
                break;
            }
            lines.push(line);
        }

        Self::from_lines(lines)
    }

    pub fn from_string(request: &str) -> Result<HttpRequest> {
        let lines = request.split(CRLF).map(|s| s.to_string()).collect();
        Self::from_lines(lines)
    }
    pub fn from_lines(lines: Vec<String>) -> Result<Self> {
        let header = RequestLine::from_line(&lines[0])?;
        let mut headers = HashMap::new();
        for line in lines.iter().skip(1) {
            if line.is_empty() {
                continue;
            }
            let mut parts = line.split(": ");
            let key = parts
                .next()
                .ok_or(ServerError::HttpError(HttpError::MissingMethod))?
                .to_string();
            let value = parts
                .next()
                .ok_or(ServerError::HttpError(HttpError::MissingMethod))?
                .to_string();
            headers.insert(key, value);
        }
        Ok(Self::new(header, headers))
    }
}

pub struct StatusCode(u16);

impl StatusCode {
    pub const OK: Self = Self(200);
    pub const NOT_FOUND: Self = Self(404);

    pub fn as_u16(&self) -> u16 {
        self.0
    }

    pub fn as_str(&self) -> &str {
        match self.0 {
            200 => "200 OK",
            404 => "404 Not Found",
            _ => "500 Internal Server Error",
        }
    }
}

pub struct HttpResponse {
    pub status_code: StatusCode,
    pub body: Vec<u8>,
}

impl HttpResponse {
    fn new(status_code: StatusCode, body: &[u8]) -> Self {
        Self {
            status_code,
            body: body.to_vec(),
        }
    }

    pub fn ok(body: &[u8]) -> Self {
        Self::new(StatusCode::OK, body)
    }
    pub fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, b"Not Found")
    }

    pub fn write_to_stream(&self, stream: &mut TcpStream) -> Result<()> {
        let mut response = Vec::new();

        response.extend_from_slice(format!("HTTP/1.1 {}", self.status_code.as_str()).as_bytes());
        response.extend_from_slice(CRLF.as_bytes());

        response.extend_from_slice(b"Content-Type: text/plain");
        response.extend_from_slice(CRLF.as_bytes());

        response.extend_from_slice(format!("Content-Length: {}", self.body.len()).as_bytes());
        response.extend_from_slice(CRLF.as_bytes());

        response.extend_from_slice(&self.body);
        response.extend_from_slice(CRLF.as_bytes());
        response.extend_from_slice(CRLF.as_bytes());

        stream.write_all(&response)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn header_from_line() {
        let request = "GET /index.html HTTP/1.1\r\nHost: localhost:4221\r\nUser-Agent: curl/7.64.1\r\nAccept: */*\r\n\r\n";
        let header = RequestLine::from_line(request).unwrap();
        let expected_header = RequestLine::new("GET", "/index.html", "HTTP/1.1");
        assert_eq!(header, expected_header);
    }

    #[test]
    fn request_from_lines() {
        let request = "GET /index.html HTTP/1.1\r\nHost: localhost:4221\r\nUser-Agent: curl/7.64.1\r\nAccept: */*\r\n\r\n";
        let lines = request.split(CRLF).map(|s| s.to_string()).collect();
        let http_request = super::HttpRequest::from_lines(lines).unwrap();
        let expected_header = RequestLine::new("GET", "/index.html", "HTTP/1.1");
        assert_eq!(http_request.request_line, expected_header);

        let mut expected_headers = std::collections::HashMap::new();
        expected_headers.insert("Host".to_string(), "localhost:4221".to_string());
        expected_headers.insert("User-Agent".to_string(), "curl/7.64.1".to_string());
        expected_headers.insert("Accept".to_string(), "*/*".to_string());
        assert_eq!(http_request.request_headers, expected_headers);
    }
}
