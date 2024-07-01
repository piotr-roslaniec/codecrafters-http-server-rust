use crate::error::{HttpError, Result, ServerError};

#[derive(Debug, PartialEq)]
pub struct Header {
    pub method: String,
    pub path: String,
    pub version: String,
}

impl Header {
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

#[cfg(test)]
mod test {
    use crate::http::Header;

    #[test]
    fn header_from_line() {
        let request = "GET /index.html HTTP/1.1\r\nHost: localhost:4221\r\nUser-Agent: curl/7.64.1\r\nAccept: */*\r\n\r\n";
        let header = Header::from_line(request).unwrap();
        let expected_header = Header::new("GET", "/index.html", "HTTP/1.1");
        assert_eq!(header, expected_header);
    }
}
