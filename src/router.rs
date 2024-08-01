use std::collections::HashMap;

use eyre::Result;

use crate::http::{
    HttpRequest, HttpResponse, ResponseHeaders, StatusCode, ACCEPT_ENCODING, CONTENT_ENCODING,
    CONTENT_TYPE, CT_APPLICATION_OCTET_STREAM, CT_TEXT_PLAIN, ENCODING_GZIP, METHOD_GET,
    METHOD_POST, USER_AGENT,
};

/// Represents a router that handles HTTP requests.
pub struct Router {
    routes: Vec<Route>,
}

impl Router {
    /// Creates a new `Router`.
    pub fn new() -> Self { Self { routes: Vec::new() } }

    /// Adds a route to the router.
    ///
    /// # Arguments
    ///
    /// * `route` - The route to add.
    pub fn add_route(&mut self, route: Route) { self.routes.push(route); }

    /// Creates a route with a handler function.
    ///
    /// # Arguments
    ///
    /// * `path` - The path for the route.
    /// * `handler` - The handler function for the route.
    pub fn create_route<F>(&mut self, path: &str, handler: F)
    where F: Fn(&HttpRequest) -> HttpResponse + Send + Sync + 'static {
        self.add_route(Route::new(path, Box::new(handler)));
    }

    /// Parses the path from a URL.
    ///
    /// # Arguments
    ///
    /// * `path` - The URL path to parse.
    ///
    /// # Returns
    ///
    /// The parsed path as a string.
    pub fn parse_path(&self, path: &str) -> String {
        let path_without_prefix = path.trim_start_matches('/');
        path_without_prefix.split('/').next().unwrap_or_default().to_string()
    }

    /// Resolves an HTTP request to a response.
    ///
    /// # Arguments
    ///
    /// * `request` - The HTTP request to resolve.
    ///
    /// # Returns
    ///
    /// A `Result` containing the HTTP response or an error.
    pub fn resolve(&self, request: &HttpRequest) -> Result<HttpResponse> {
        for route in &self.routes {
            if self.parse_path(&request.line.path) == self.parse_path(&route.path) {
                return Ok((route.handler)(request));
            }
        }
        Ok(HttpResponse::not_found())
    }
}

pub type RouteHandler = Box<dyn Fn(&HttpRequest) -> HttpResponse + Send + Sync>;

/// Represents a route in the router.
pub struct Route {
    path:    String,
    handler: RouteHandler,
}

impl Route {
    /// Creates a new `Route`.
    ///
    /// # Arguments
    ///
    /// * `path` - The path for the route.
    /// * `handler` - The handler for the route.
    ///
    /// # Returns
    ///
    /// A new `Route` instance.
    pub fn new(path: &str, handler: RouteHandler) -> Self {
        Self { path: path.to_string(), handler }
    }
}

/// Creates a router with predefined routes.
///
/// # Arguments
///
/// * `pub_dir` - The public directory for file routes.
///
/// # Returns
///
/// A `Router` instance.
pub fn make_router(pub_dir: &str) -> Router {
    let pub_dir = pub_dir.to_string();
    let mut router = Router::new();

    router.create_route("/", |request| match request.line.method.as_str() {
        METHOD_GET => HttpResponse::ok(b"", ResponseHeaders::new()),
        _ => HttpResponse::method_not_allowed(),
    });

    router.create_route("/echo", move |request| match request.line.method.as_str() {
        METHOD_GET => {
            let path_without_prefix = request.line.path.trim_start_matches("/echo/");
            let mut headers = ResponseHeaders::new();
            headers.insert(CONTENT_TYPE.to_string(), CT_TEXT_PLAIN.to_string());
            accept_encoding(request, &mut headers);
            HttpResponse::ok(path_without_prefix.as_bytes(), headers)
        },
        _ => HttpResponse::method_not_allowed(),
    });

    router.create_route("/user-agent", move |request| match request.line.method.as_str() {
        METHOD_GET => {
            let default = String::new();
            let user_agent = request.headers.get(USER_AGENT).unwrap_or(&default);
            let mut headers = ResponseHeaders::new();
            headers.insert(CONTENT_TYPE.to_string(), CT_TEXT_PLAIN.to_string());
            accept_encoding(request, &mut headers);
            HttpResponse::ok(user_agent.as_bytes(), headers)
        },
        _ => HttpResponse::method_not_allowed(),
    });

    router.create_route("/files", move |request| {
        let mut headers = ResponseHeaders::new();
        headers.insert(CONTENT_TYPE.to_string(), CT_APPLICATION_OCTET_STREAM.to_string());
        accept_encoding(request, &mut headers);
        let file = request.line.path.trim_start_matches("/files/");
        let file = format!("{}/{}", pub_dir, file);
        match request.line.method.as_str() {
            METHOD_GET => match std::fs::read(&file) {
                Ok(body) => HttpResponse::new(StatusCode::OK, &body, headers),
                Err(_) => HttpResponse::not_found(),
            },
            METHOD_POST => {
                let request_body = request.body.clone();
                match std::fs::write(&file, request_body) {
                    Ok(_) => HttpResponse::created(),
                    Err(_) => HttpResponse::internal_server_error(),
                }
            },
            _ => HttpResponse::method_not_allowed(),
        }
    });

    router
}

/// Adds the appropriate encoding to the response headers based on the request.
///
/// # Arguments
///
/// * `request` - The HTTP request.
/// * `headers` - The response headers to modify.
fn accept_encoding(request: &HttpRequest, headers: &mut HashMap<String, String>) {
    if let Some(encoding_str) = request.headers.get(ACCEPT_ENCODING) {
        let encodings = encoding_str.split(", ").map(|s| s.trim()).filter(|s| !s.is_empty());
        for encoding in encodings {
            if encoding == ENCODING_GZIP {
                headers.insert(CONTENT_ENCODING.to_string(), encoding.to_string());
            }
        }
    }
}

#[cfg(test)]
mod test {
    use tempdir::TempDir;

    use super::*;
    use crate::http::StatusCode;

    const TEST_PUBLIC_DIR: &str = "/tmp/test_public";

    fn make_test_router() -> Router { make_router(TEST_PUBLIC_DIR) }

    #[test]
    fn test_router_parse_path() {
        let router = make_test_router();
        assert_eq!(router.parse_path("/"), "");
        assert_eq!(router.parse_path("/echo"), "echo");
        assert_eq!(router.parse_path("/echo/"), "echo");
        assert_eq!(router.parse_path("/echo/123"), "echo");
    }

    #[test]
    fn test_router_resolve_root() {
        let router = make_test_router();
        let request = HttpRequest::from_string(
            "GET / HTTP/1.1\r\nHost: localhost:4221\r\nUser-Agent: curl/7.64.1\r\nAccept: \
             */*\r\n\r\n",
        )
        .unwrap();
        let response = router.resolve(&request).unwrap();
        assert_eq!(response.status_code, StatusCode::OK);
        assert_eq!(response.body, b"");
        assert_eq!(response.to_bytes().unwrap(), b"HTTP/1.1 200 OK\r\n\r\n");
    }

    #[test]
    fn test_router_resolve_echo() {
        let expected_body = "my_test_path";
        let router = make_test_router();
        let request = HttpRequest::from_string(&format!(
            "GET /echo/{} HTTP/1.1\r\nHost: localhost:4221\r\nUser-Agent: curl/7.64.1\r\nAccept: \
             */*\r\n\r\n",
            expected_body
        ))
        .unwrap();
        let response = router.resolve(&request).unwrap();
        assert_eq!(response.status_code, StatusCode::OK);
        assert_eq!(response.body, expected_body.as_bytes());
        assert_eq!(
            response.to_bytes().unwrap(),
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                expected_body.len(),
                expected_body
            )
            .as_bytes()
        );
    }

    #[test]
    fn test_router_resolve_not_found() {
        let router = make_test_router();
        let request = HttpRequest::from_string(
            "GET /not_found HTTP/1.1\r\nHost: localhost:4221\r\nUser-Agent: \
             curl/7.64.1\r\nAccept: */*\r\n\r\n",
        )
        .unwrap();
        let response = router.resolve(&request).unwrap();
        assert_eq!(response.status_code, StatusCode::NOT_FOUND);
        assert_eq!(response.body, b"");
        assert_eq!(response.to_bytes().unwrap(), b"HTTP/1.1 404 Not Found\r\n\r\n");
    }

    #[test]
    fn test_example() {
        let router = make_test_router();
        let request =
            HttpRequest::from_string("GET / HTTP/1.1\r\nHost: localhost:4221\r\n\r\n").unwrap();
        let response = router.resolve(&request).unwrap();
        assert_eq!(response.status_code, StatusCode::OK);
        assert_eq!(response.body, b"");
        assert_eq!(response.to_bytes().unwrap(), b"HTTP/1.1 200 OK\r\n\r\n");
    }

    #[test]
    fn test_echo_example() {
        let router = make_test_router();
        let request = HttpRequest::from_string(
            "GET /echo/abc HTTP/1.1\r\nHost: localhost:4221\r\nUser-Agent: curl/7.64.1\r\nAccept: \
             */*\r\n\r\n",
        )
        .unwrap();
        let response = router.resolve(&request).unwrap();
        assert_eq!(response.status_code, StatusCode::OK);
        assert_eq!(response.body, b"abc");
        assert_eq!(
            response.to_bytes().unwrap(),
            b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 3\r\n\r\nabc"
        );
    }

    #[test]
    fn test_user_agent() {
        let router = make_test_router();
        let user_agent = "banana/blueberry";
        let request = HttpRequest::from_string(&format!(
            "GET /user-agent HTTP/1.1\r\nHost: localhost:4221\r\nUser-Agent: {}\r\nAccept: \
             */*\r\n\r\n",
            user_agent
        ))
        .unwrap();
        let response = router.resolve(&request).unwrap();
        assert_eq!(response.status_code, StatusCode::OK);
        assert_eq!(response.body, user_agent.as_bytes());
        assert_eq!(
            response.to_bytes().unwrap(),
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                user_agent.len(),
                user_agent
            )
            .as_bytes()
        );
    }

    #[test]
    fn test_files() {
        let tmp_dir = TempDir::new("test_files").unwrap();
        let file_path = tmp_dir.path().join("test.txt");
        let contents = "test";
        std::fs::write(file_path, contents).unwrap();
        let tmp_dir = tmp_dir.path().to_str().unwrap();

        let router = make_router(tmp_dir);
        let request = HttpRequest::from_string(
            "GET /files/test.txt HTTP/1.1\r\nHost: localhost:4221\r\nUser-Agent: \
             curl/7.64.1\r\nAccept: */*\r\n\r\n",
        )
        .unwrap();
        let response = router.resolve(&request).unwrap();
        assert_eq!(response.status_code, StatusCode::OK);
        assert_eq!(response.body, contents.as_bytes());
        assert_eq!(
            response.to_string().unwrap(),
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: \
                 4\r\n\r\n{}",
                contents
            )
        );
    }

    #[test]
    fn test_files_file_not_exists() {
        let tmp_dir = TempDir::new("test_files").unwrap();
        let tmp_dir = tmp_dir.path().to_str().unwrap();

        let router = make_router(tmp_dir);
        let request = HttpRequest::from_string(
            "GET /files/test.txt HTTP/1.1\r\nHost: localhost:4221\r\nUser-Agent: \
             curl/7.64.1\r\nAccept: */*\r\n\r\n",
        )
        .unwrap();
        let response = router.resolve(&request).unwrap();
        assert_eq!(response.status_code, StatusCode::NOT_FOUND);
        assert_eq!(response.body, b"");
        assert_eq!(response.to_bytes().unwrap(), b"HTTP/1.1 404 Not Found\r\n\r\n");
    }

    #[test]
    fn test_files_post() {
        let tmp_dir = TempDir::new("test_files").unwrap();
        let file_path = tmp_dir.path().join("test.txt");
        let tmp_dir = tmp_dir.path().to_str().unwrap();

        let router = make_router(tmp_dir);
        let request = HttpRequest::from_string(
            "POST /files/test.txt HTTP/1.1\r\nHost: localhost:4221\r\nUser-Agent: \
             curl/7.64.1\r\nContent-Length: 4\r\nContent-Type: \
             application/octet-stream\r\nAccept: */*\r\n\r\ntest",
        )
        .unwrap();
        let response = router.resolve(&request).unwrap();
        assert_eq!(response.status_code, StatusCode::CREATED);
        assert_eq!(response.body, b"");
        assert_eq!(response.to_bytes().unwrap(), b"HTTP/1.1 201 Created\r\n\r\n");
        assert_eq!(std::fs::read_to_string(file_path).unwrap(), "test");
    }

    #[test]
    fn test_post_file_example() {
        let contents = "mango banana mango grape blueberry orange banana grape";
        let filename = "strawberry_blueberry_raspberry_raspberry";
        let tmp_dir = TempDir::new("test_files").unwrap();
        let file_path = tmp_dir.path().join(filename);
        let tmp_dir = tmp_dir.path().to_str().unwrap();

        let request_str = format!(
            "POST /files/{} HTTP/1.1\r\nHost: localhost:4221\r\nContent-Length: \
             {}\r\nContent-Type: application/octet-stream\r\n\r\n{}",
            filename,
            contents.len(),
            contents
        );
        let request = HttpRequest::from_string(&request_str).unwrap();
        let router = make_router(tmp_dir);
        let response = router.resolve(&request).unwrap();
        assert_eq!(response.status_code, StatusCode::CREATED);
        assert_eq!(response.body, b"");
        assert_eq!(response.to_bytes().unwrap(), b"HTTP/1.1 201 Created\r\n\r\n");
        assert_eq!(std::fs::read_to_string(file_path).unwrap(), contents);
    }
}
