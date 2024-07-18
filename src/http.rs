use crate::error::{HttpError, ServerError};
use eyre::Result;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::collections::HashMap;
use std::io::Write;

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
    pub connection: String,
    pub body: Vec<u8>,
}

impl HttpRequest {
    fn new(line: RequestLine, headers: RequestHeaders, body: Vec<u8>) -> Self {
        let connection = headers
            .get("Connection")
            .unwrap_or(&"keep-alive".to_string())
            .to_owned();
        Self {
            line,
            headers,
            connection,
            body,
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let string = String::from_utf8_lossy(bytes);
        Self::from_string(&string)
    }
    pub fn from_string(string: &str) -> Result<Self> {
        let lines = string
            .split(CRLF)
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        Self::from_strings(&lines)
    }

    pub fn from_strings(strings: &[String]) -> Result<Self> {
        let request_line = strings
            .first()
            .ok_or(ServerError::HttpError(HttpError::MissingRequestLine))?;
        let headers = strings
            .iter()
            // Skip request line
            .skip(1)
            // Take headers until an empty line is found
            .take_while(|line| !line.is_empty())
            .cloned()
            .collect::<Vec<_>>();
        let body = strings
            .iter()
            // Skip request line, headers and empty line
            .skip(1 + headers.len() + 1)
            .cloned()
            .collect::<Vec<_>>();
        Self::from_lines(request_line, &headers, &body)
    }

    pub fn from_lines(
        request_line: &str,
        header_lines: &[String],
        body_lines: &[String],
    ) -> Result<Self> {
        let line = RequestLine::from_line(request_line)?;

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

        // Check for Content-Length header
        let content_length = headers
            .get("Content-Length")
            .map(|s| s.parse::<usize>())
            .transpose()
            .map_err(|_| ServerError::HttpError(HttpError::InvalidContentLength))?
            .unwrap_or(0);

        if content_length != 0 {
            // Make sure the Content-Type header is present and set to "application/octet-stream"
            // if !headers.contains_key("Content-Type") {
            //     return Err(ServerError::HttpError(HttpError::MissingHeaderValue).into());
            // } else {
            //     let content_type = headers.get("Content-Type").unwrap();
            //     if content_type != "application/octet-stream" {
            //         return Err(ServerError::HttpError(HttpError::InvalidContentType).into());
            //     }
            // }

            let body: Vec<u8> = body_lines
                .iter()
                .flat_map(|line| line.as_bytes())
                .copied()
                .collect();
            if body.len() != content_length {
                return Err(ServerError::HttpError(HttpError::InvalidContentLength).into());
            }
            return Ok(Self::new(line, headers, body));
        }

        Ok(Self::new(line, headers, Vec::new()))
    }
}

#[derive(Debug, PartialEq)]
pub struct StatusCode(u16);

impl StatusCode {
    pub const OK: Self = Self(200);
    pub const CREATED: Self = Self(201);
    pub const NOT_FOUND: Self = Self(404);
    pub const NOT_ALLOWED: Self = Self(405);
    pub const INTERNAL_SERVER_ERROR: Self = Self(500);

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

#[derive(Debug)]
pub struct HttpResponse {
    pub status_code: StatusCode,
    pub headers: ResponseHeaders,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn new(status_code: StatusCode, body: &[u8], headers: ResponseHeaders) -> Self {
        Self {
            status_code,
            headers,
            body: body.to_vec(),
        }
    }

    pub fn ok(body: &[u8], headers: ResponseHeaders) -> Self {
        Self::new(StatusCode::OK, body, headers)
    }

    pub fn created() -> Self {
        Self::new(StatusCode::CREATED, b"", ResponseHeaders::new())
    }
    pub fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, b"", ResponseHeaders::new())
    }

    pub fn method_not_allowed() -> Self {
        let mut headers = ResponseHeaders::new();
        headers.insert("Allow".to_string(), "GET".to_string());
        Self::new(StatusCode::NOT_ALLOWED, b"", headers)
    }

    pub fn internal_server_error() -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            b"",
            ResponseHeaders::new(),
        )
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut response = Vec::new();

        response.extend_from_slice(format!("HTTP/1.1 {}", self.status_code.as_str()).as_bytes());
        response.extend_from_slice(CRLF.as_bytes());

        for (key, header) in &self.headers {
            response.extend_from_slice(key.as_bytes());
            response.extend_from_slice(b": ");
            response.extend_from_slice(header.as_bytes());
            response.extend_from_slice(CRLF.as_bytes());
        }

        if !self.body.is_empty() {
            let content_encoding = self.headers.get("Content-Encoding");
            let body = if content_encoding.is_some() {
                match content_encoding.unwrap().as_str() {
                    "gzip" => {
                        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
                        encoder.write_all(&self.body)?;
                        encoder.finish()?
                    }
                    _ => self.body.clone(),
                }
            } else {
                self.body.clone()
            };

            response.extend_from_slice(format!("Content-Length: {}", body.len()).as_bytes());
            response.extend_from_slice(CRLF.as_bytes());

            // End of headers
            response.extend_from_slice(CRLF.as_bytes());

            // Body
            response.extend_from_slice(&body);
        } else {
            // End of headers, no body
            response.extend_from_slice(CRLF.as_bytes());
        }
        Ok(response)
    }

    pub fn to_string(&self) -> Result<String> {
        Ok(String::from_utf8_lossy(&self.to_bytes()?).to_string())
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
        let expected_header = RequestLine::new("GET", "/index.html", "HTTP/1.1");
        assert_eq!(http_request.line, expected_header);
        assert_eq!(http_request.body, body.clone().into_bytes());

        let mut expected_headers = HashMap::new();
        expected_headers.insert("Host".to_string(), "localhost:4221".to_string());
        expected_headers.insert("User-Agent".to_string(), "curl/7.64.1".to_string());
        expected_headers.insert("Accept".to_string(), "*/*".to_string());
        expected_headers.insert("Content-Length".to_string(), body.len().to_string());
        expected_headers.insert(
            "Content-Type".to_string(),
            "application/octet-stream".to_string(),
        );
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
