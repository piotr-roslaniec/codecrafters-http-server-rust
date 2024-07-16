use crate::error::{HttpError, ServerError};
use eyre::Result;
use std::collections::HashMap;
use std::io::BufRead;

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

pub type RequestHeaders = HashMap<String, String>;

#[derive(Debug)]
pub struct HttpRequest {
    pub line: RequestLine,
    pub headers: RequestHeaders,
}

impl HttpRequest {
    fn new(line: RequestLine, headers: RequestHeaders) -> Self {
        Self { line, headers }
    }

    pub fn from_string(request: &str) -> Result<HttpRequest> {
        let lines = request.split(CRLF).map(|s| s.to_string()).collect();
        Self::from_lines(&lines)
    }
    pub fn from_lines(lines: &Vec<String>) -> Result<Self> {
        let header = RequestLine::from_line(&lines[0])?;
        let mut headers = HashMap::new();
        for line in lines.iter().skip(1) {
            let line = line.trim_end();
            if line.is_empty() {
                continue;
            }
            let mut parts = line.split(": ");
            let key = parts
                .next()
                .ok_or(ServerError::HttpError(HttpError::MissingHeaderKey))?
                .to_string();
            let value = parts
                .next()
                .ok_or(ServerError::HttpError(HttpError::MissingHeaderValue))?
                .to_string();
            headers.insert(key, value);
        }
        Ok(Self::new(header, headers))
    }
}

#[derive(Debug, PartialEq)]
pub struct StatusCode(u16);

impl StatusCode {
    pub const OK: Self = Self(200);
    pub const NOT_FOUND: Self = Self(404);

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
        Self::new(StatusCode::NOT_FOUND, b"")
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut response = Vec::new();

        response.extend_from_slice(format!("HTTP/1.1 {}", self.status_code.as_str()).as_bytes());
        response.extend_from_slice(CRLF.as_bytes());

        response.extend_from_slice(b"Content-Type: text/plain");
        response.extend_from_slice(CRLF.as_bytes());

        response.extend_from_slice(format!("Content-Length: {}", self.body.len()).as_bytes());
        response.extend_from_slice(CRLF.as_bytes());

        // End of headers
        response.extend_from_slice(CRLF.as_bytes());

        if !self.body.is_empty() {
            response.extend_from_slice(&self.body);
        }
        response
    }

    pub fn to_string(&self) -> Result<String> {
        Ok(String::from_utf8(self.to_bytes())?.to_string())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn request_from_line() {
        let request = "GET /index.html HTTP/1.1\r\nHost: localhost:4221\r\nUser-Agent: curl/7.64.1\r\nAccept: */*\r\n\r\n";
        let header = RequestLine::from_line(request).unwrap();
        let expected_header = RequestLine::new("GET", "/index.html", "HTTP/1.1");
        assert_eq!(header, expected_header);
    }

    #[test]
    fn request_from_lines() {
        let lines = vec![
            "GET /index.html HTTP/1.1".to_string(),
            "Host: localhost:4221".to_string(),
            "User-Agent: curl/7.64.1".to_string(),
            "Accept: */*".to_string(),
        ];
        let http_request = super::HttpRequest::from_lines(&lines).unwrap();
        let expected_header = RequestLine::new("GET", "/index.html", "HTTP/1.1");
        assert_eq!(http_request.line, expected_header);

        let mut expected_headers = HashMap::new();
        expected_headers.insert("Host".to_string(), "localhost:4221".to_string());
        expected_headers.insert("User-Agent".to_string(), "curl/7.64.1".to_string());
        expected_headers.insert("Accept".to_string(), "*/*".to_string());
        assert_eq!(http_request.headers, expected_headers);
    }

    #[test]
    fn response_to_bytes() {
        let response = HttpResponse::ok(b"");
        assert_eq!(
            response.to_bytes(),
            b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 0\r\n\r\n"
        );

        let response = HttpResponse::ok(b"Hello, world!");
        assert_eq!(
            response.to_bytes(),
            b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 13\r\n\r\nHello, world!"
        );
    }
}
