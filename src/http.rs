use std::{collections::HashMap, io::Write};

use eyre::Result;
use flate2::{write::GzEncoder, Compression};

use crate::error::{HttpError, ServerError};

const CRLF: &str = "\r\n";
const HTTP_VERSION_1_1: &str = "HTTP/1.1";

pub const KEEP_ALIVE: &str = "keep-alive";
pub const CONTENT_ENCODING: &str = "Content-Encoding";
pub const ACCEPT_ENCODING: &str = "Accept-Encoding";
pub const ENCODING_GZIP: &str = "gzip";
pub const CONTENT_LENGTH: &str = "Content-Length";
pub const CONNECTION: &str = "Connection";
pub const CONTENT_TYPE: &str = "Content-Type";
pub const CT_TEXT_PLAIN: &str = "text/plain";
pub const USER_AGENT: &str = "User-Agent";
pub const CT_APPLICATION_OCTET_STREAM: &str = "application/octet-stream";

pub const METHOD_GET: &str = "GET";
pub const METHOD_POST: &str = "POST";

/// Represents the request line of an HTTP request.
#[derive(Debug, PartialEq)]
pub struct RequestLine {
    pub method:  String,
    pub path:    String,
    pub version: String,
}

impl RequestLine {
    /// Creates a new `RequestLine`.
    pub fn new(method: &str, path: &str, version: &str) -> Self {
        Self {
            method:  method.to_string(),
            path:    path.to_string(),
            version: version.to_string(),
        }
    }

    /// Parses a request line from a string.
    pub fn from_line(line: &str) -> Result<Self> {
        let mut iter = line.split_whitespace();
        let method =
            iter.next().ok_or(ServerError::HttpError(HttpError::MissingMethod))?.to_string();
        let path = iter.next().ok_or(ServerError::HttpError(HttpError::MissingPath))?.to_string();
        let version =
            iter.next().ok_or(ServerError::HttpError(HttpError::MissingVersion))?.to_string();
        if version != HTTP_VERSION_1_1 {
            return Err(ServerError::HttpError(HttpError::UnsupportedVersion).into());
        }
        Ok(Self { method, path, version })
    }
}

pub type RequestHeaders = HashMap<String, String>;

/// Represents an HTTP request.
#[derive(Debug)]
pub struct HttpRequest {
    pub line:       RequestLine,
    pub headers:    RequestHeaders,
    pub connection: String,
    pub body:       Vec<u8>,
}

impl HttpRequest {
    /// Creates a new `HttpRequest`.
    fn new(line: RequestLine, headers: RequestHeaders, body: Vec<u8>) -> Self {
        let connection = headers.get(CONNECTION).unwrap_or(&KEEP_ALIVE.to_string()).to_owned();
        Self { line, headers, connection, body }
    }

    /// Parses an HTTP request from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let string = String::from_utf8_lossy(bytes);
        Self::from_string(&string)
    }

    /// Parses an HTTP request from a string.
    pub fn from_string(string: &str) -> Result<Self> {
        let lines = string.split(CRLF).map(|s| s.to_string()).collect::<Vec<_>>();
        Self::from_strings(&lines)
    }

    /// Parses an HTTP request from a list of strings.
    pub fn from_strings(strings: &[String]) -> Result<Self> {
        let request_line =
            strings.first().ok_or(ServerError::HttpError(HttpError::MissingRequestLine))?;
        let headers =
            strings.iter().skip(1).take_while(|line| !line.is_empty()).cloned().collect::<Vec<_>>();
        let body = strings.iter().skip(1 + headers.len() + 1).cloned().collect::<Vec<_>>();
        Self::from_lines(request_line, &headers, &body)
    }

    /// Parses an HTTP request from request line, headers, and body.
    pub fn from_lines(
        request_line: &str,
        header_lines: &[String],
        body_lines: &[String],
    ) -> Result<Self> {
        let line = RequestLine::from_line(request_line)?;
        let headers = Self::parse_headers(header_lines)?;
        let body = Self::parse_body(&headers, body_lines)?;
        Ok(Self::new(line, headers, body))
    }

    /// Parses headers from a list of strings.
    fn parse_headers(header_lines: &[String]) -> Result<RequestHeaders> {
        let mut headers = HashMap::new();
        for line in header_lines {
            if line.is_empty() {
                break;
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
        Ok(headers)
    }

    /// Parses the body from a list of strings.
    fn parse_body(headers: &RequestHeaders, body_lines: &[String]) -> Result<Vec<u8>> {
        let content_length = headers
            .get(CONTENT_LENGTH)
            .map(|s| s.parse::<usize>())
            .transpose()
            .map_err(|_| ServerError::HttpError(HttpError::InvalidContentLength))?
            .unwrap_or(0);

        if content_length != 0 {
            let body: Vec<u8> =
                body_lines.iter().flat_map(|line| line.as_bytes()).copied().collect();
            if body.len() != content_length {
                return Err(ServerError::HttpError(HttpError::InvalidContentLength).into());
            }
            return Ok(body);
        }
        Ok(Vec::new())
    }
}

/// Represents an HTTP status code.
#[derive(Debug, PartialEq)]
pub struct StatusCode(u16);

impl StatusCode {
    pub const CREATED: Self = Self(201);
    pub const INTERNAL_SERVER_ERROR: Self = Self(500);
    pub const NOT_ALLOWED: Self = Self(405);
    pub const NOT_FOUND: Self = Self(404);
    pub const OK: Self = Self(200);

    /// Returns the status code as a string.
    pub fn as_str(&self) -> &str {
        match self.0 {
            200 => "200 OK",
            201 => "201 Created",
            404 => "404 Not Found",
            405 => "405 Method Not Allowed",
            500 => "500 Internal Server Error",
            _ => "500 Internal Server Error",
        }
    }
}

pub type ResponseHeaders = HashMap<String, String>;

/// Represents an HTTP response.
#[derive(Debug)]
pub struct HttpResponse {
    pub status_code: StatusCode,
    pub headers:     ResponseHeaders,
    pub body:        Vec<u8>,
}

impl HttpResponse {
    /// Creates a new `HttpResponse`.
    pub fn new(status_code: StatusCode, body: &[u8], headers: ResponseHeaders) -> Self {
        Self { status_code, headers, body: body.to_vec() }
    }

    /// Creates a 200 OK response.
    pub fn ok(body: &[u8], headers: ResponseHeaders) -> Self {
        Self::new(StatusCode::OK, body, headers)
    }

    /// Creates a response from a status code.
    pub fn from_status_code(status_code: StatusCode) -> Self {
        Self::new(status_code, b"", ResponseHeaders::new())
    }

    /// Creates a 201 Created response.
    pub fn created() -> Self { Self::from_status_code(StatusCode::CREATED) }

    /// Creates a 404 Not Found response.
    pub fn not_found() -> Self { Self::from_status_code(StatusCode::NOT_FOUND) }

    /// Creates a 405 Method Not Allowed response.
    pub fn method_not_allowed() -> Self { Self::from_status_code(StatusCode::NOT_ALLOWED) }

    /// Creates a 500 Internal Server Error response.
    pub fn internal_server_error() -> Self {
        Self::from_status_code(StatusCode::INTERNAL_SERVER_ERROR)
    }

    /// Serializes the response to bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut response = Vec::new();
        response.extend_from_slice(
            format!("{} {}", HTTP_VERSION_1_1, self.status_code.as_str()).as_bytes(),
        );
        response.extend_from_slice(CRLF.as_bytes());

        // Serialize headers
        for (key, header) in &self.headers {
            response.extend_from_slice(key.as_bytes());
            response.extend_from_slice(b": ");
            response.extend_from_slice(header.as_bytes());
            response.extend_from_slice(CRLF.as_bytes());
        }

        // Serialize body
        if self.body.is_empty() {
            response.extend_from_slice(CRLF.as_bytes());
            return Ok(response);
        }

        let body = self.encode_body_content()?;
        response.extend_from_slice(format!("{}: {}", CONTENT_LENGTH, body.len()).as_bytes());
        response.extend_from_slice(CRLF.as_bytes());

        // End of headers
        response.extend_from_slice(CRLF.as_bytes());

        // Body
        response.extend_from_slice(&body);

        Ok(response)
    }

    /// Compresses the body if necessary.
    fn encode_body_content(&self) -> Result<Vec<u8>> {
        if let Some(content_encoding) = self.headers.get(CONTENT_ENCODING) {
            if content_encoding == ENCODING_GZIP {
                let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
                encoder.write_all(&self.body)?;
                return Ok(encoder.finish()?);
            }
        }
        Ok(self.body.clone())
    }

    /// Serializes the response to a string.
    pub fn to_string(&self) -> Result<String> {
        Ok(String::from_utf8_lossy(&self.to_bytes()?).to_string())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn request_from_line() {
        let request = "GET /index.html HTTP/1.1\r\nHost: localhost:4221\r\nUser-Agent: \
                       curl/7.64.1\r\nAccept: */*\r\n\r\n";
        let header = RequestLine::from_line(request).unwrap();
        let expected_header = RequestLine::new(METHOD_GET, "/index.html", HTTP_VERSION_1_1);
        assert_eq!(header, expected_header);
    }

    #[test]
    fn request_from_lines() {
        let body = "Hello, world!".to_string();
        let request_line = "GET /index.html HTTP/1.1";
        let header_lines = vec![
            "Host: localhost:4221".to_string(),
            "User-Agent: curl/7.64.1".to_string(),
            "Accept: */*".to_string(),
            "Content-Type: application/octet-stream".to_string(),
            format!("Content-Length: {}", body.len()).to_string(),
        ];
        let body_lines = vec![body.clone()];
        let http_request =
            HttpRequest::from_lines(request_line, &header_lines, &body_lines).unwrap();
        let expected_header = RequestLine::new(METHOD_GET, "/index.html", HTTP_VERSION_1_1);
        assert_eq!(http_request.line, expected_header);
        assert_eq!(http_request.body, body.clone().into_bytes());

        let mut expected_headers = HashMap::new();
        expected_headers.insert("Host".to_string(), "localhost:4221".to_string());
        expected_headers.insert("User-Agent".to_string(), "curl/7.64.1".to_string());
        expected_headers.insert("Accept".to_string(), "*/*".to_string());
        expected_headers.insert(CONTENT_LENGTH.to_string(), body.len().to_string());
        expected_headers.insert("Content-Type".to_string(), "application/octet-stream".to_string());
        assert_eq!(http_request.headers, expected_headers);
    }

    #[test]
    fn response_to_bytes() {
        let response = HttpResponse::ok(b"", ResponseHeaders::new());
        assert_eq!(response.to_bytes().unwrap(), b"HTTP/1.1 200 OK\r\n\r\n");

        let headers = {
            let mut headers = HashMap::new();
            headers.insert("Content-Type".to_string(), "text/plain".to_string());
            headers
        };
        let response = HttpResponse::ok(b"Hello, world!", headers);
        assert_eq!(
            response.to_bytes().unwrap(),
            b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 13\r\n\r\nHello, world!"
        );
    }
}
