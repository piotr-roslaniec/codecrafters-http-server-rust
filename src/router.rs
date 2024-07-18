use crate::http::{HttpRequest, HttpResponse, ResponseHeaders, StatusCode};
use eyre::Result;
use std::collections::HashMap;

pub struct Router {
    routes: Vec<Route>,
}

impl Router {
    pub fn new() -> Self {
        Self { routes: Vec::new() }
    }

    pub fn add_route(&mut self, route: Route) {
        self.routes.push(route);
    }

    pub fn parse_path(&self, path: &str) -> String {
        let path_without_prefix = path.trim_start_matches('/');
        path_without_prefix
            .split('/')
            .next()
            .unwrap_or_default()
            .to_string()
    }

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

pub struct Route {
    path: String,
    handler: RouteHandler,
}

impl Route {
    pub fn new(path: &str, handler: RouteHandler) -> Self {
        Self {
            path: path.to_string(),
            handler,
        }
    }
}

pub fn make_router(directory: Option<String>) -> Router {
    let mut router = Router::new();

    let default_route = Route::new(
        "/",
        Box::new(|request| {
            if request.line.method == "GET" {
                HttpResponse::ok(b"", ResponseHeaders::new())
            } else {
                HttpResponse::method_not_allowed()
            }
        }),
    );
    let echo_route = Route::new(
        "/echo",
        Box::new(move |request| {
            if request.line.method != "GET" {
                return HttpResponse::method_not_allowed();
            }
            let path_without_prefix = request.line.path.trim_start_matches("/echo/");

            let mut headers = ResponseHeaders::new();
            headers.insert("Content-Type".to_string(), "text/plain".to_string());
            if let Some(response) = accept_encoding(request, &mut headers) {
                return response;
            }

            HttpResponse::ok(path_without_prefix.as_bytes(), headers)
        }),
    );
    let user_agent_route = Route::new(
        "/user-agent",
        Box::new(move |request| {
            if request.line.method != "GET" {
                return HttpResponse::method_not_allowed();
            }
            let default = String::new();
            let user_agent = request.headers.get("User-Agent").unwrap_or(&default);

            let mut headers = ResponseHeaders::new();
            headers.insert("Content-Type".to_string(), "text/plain".to_string());
            if let Some(response) = accept_encoding(request, &mut headers) {
                return response;
            }

            HttpResponse::ok(user_agent.as_bytes(), headers)
        }),
    );
    let files_route = Route::new(
        "/files",
        Box::new(move |request| {
            let mut headers = ResponseHeaders::new();
            headers.insert(
                "Content-Type".to_string(),
                "application/octet-stream".to_string(),
            );
            if let Some(response) = accept_encoding(request, &mut headers) {
                return response;
            }

            let default_dir = "/tmp".to_string();
            let directory = directory.as_ref().unwrap_or(&default_dir);
            let file = request.line.path.trim_start_matches("/files/");
            let file = format!("{}/{}", directory, file);

            if request.line.method == "GET" {
                let body = match std::fs::read(file) {
                    Ok(contents) => contents,
                    Err(_) => return HttpResponse::not_found(),
                };
                HttpResponse::new(StatusCode::OK, &body, headers)
            } else if request.line.method == "POST" {
                let request_body = request.body.clone();
                match std::fs::write(file, request_body) {
                    Ok(_) => HttpResponse::created(),
                    Err(_) => HttpResponse::internal_server_error(),
                }
            } else {
                HttpResponse::method_not_allowed()
            }
        }),
    );

    router.add_route(default_route);
    router.add_route(echo_route);
    router.add_route(user_agent_route);
    router.add_route(files_route);
    router
}

fn accept_encoding(
    request: &HttpRequest,
    headers: &mut HashMap<String, String>,
) -> Option<HttpResponse> {
    if let Some(encoding) = request.headers.get("Accept-Encoding") {
        if encoding != "gzip" {
            return Some(HttpResponse::not_found());
        }
        headers.insert("Content-Encoding".to_string(), encoding.to_string());
    }
    None
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::http::StatusCode;
    use nom::AsBytes;
    use std::io::Write;
    use tempdir::TempDir;

    #[test]
    fn test_router_parse_path() {
        let router = make_router(None);
        assert_eq!(router.parse_path("/"), "");
        assert_eq!(router.parse_path("/echo"), "echo");
        assert_eq!(router.parse_path("/echo/"), "echo");
        assert_eq!(router.parse_path("/echo/123"), "echo");
    }

    #[test]
    fn test_router_resolve_root() {
        let router = make_router(None);
        let request = HttpRequest::from_string("GET / HTTP/1.1\r\nHost: localhost:4221\r\nUser-Agent: curl/7.64.1\r\nAccept: */*\r\n\r\n").unwrap();
        let response = router.resolve(&request).unwrap();
        assert_eq!(response.status_code, StatusCode::OK);
        assert_eq!(response.body, b"");
        assert_eq!(response.to_bytes().unwrap(), b"HTTP/1.1 200 OK\r\n\r\n");
    }

    #[test]
    fn test_router_resolve_echo() {
        let expected_body = "my_test_path";
        let router = make_router(None);
        let request = HttpRequest::from_string(&format!("GET /echo/{} HTTP/1.1\r\nHost: localhost:4221\r\nUser-Agent: curl/7.64.1\r\nAccept: */*\r\n\r\n", expected_body)).unwrap();
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
        let router = make_router(None);
        let request = HttpRequest::from_string("GET /not_found HTTP/1.1\r\nHost: localhost:4221\r\nUser-Agent: curl/7.64.1\r\nAccept: */*\r\n\r\n").unwrap();
        let response = router.resolve(&request).unwrap();
        assert_eq!(response.status_code, StatusCode::NOT_FOUND);
        assert_eq!(response.body, b"");
        assert_eq!(
            response.to_bytes().unwrap(),
            b"HTTP/1.1 404 Not Found\r\n\r\n"
        );
    }

    #[test]
    fn test_example() {
        let router = make_router(None);
        let request =
            HttpRequest::from_string("GET / HTTP/1.1\r\nHost: localhost:4221\r\n\r\n").unwrap();
        let response = router.resolve(&request).unwrap();
        assert_eq!(response.status_code, StatusCode::OK);
        assert_eq!(response.body, b"");
        assert_eq!(response.to_bytes().unwrap(), b"HTTP/1.1 200 OK\r\n\r\n");
    }

    #[test]
    fn test_echo_example() {
        let router = make_router(None);
        let request = HttpRequest::from_string("GET /echo/abc HTTP/1.1\r\nHost: localhost:4221\r\nUser-Agent: curl/7.64.1\r\nAccept: */*\r\n\r\n").unwrap();
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
        let router = make_router(None);
        let user_agent = "banana/blueberry";
        let request = HttpRequest::from_string(&format!("GET /user-agent HTTP/1.1\r\nHost: localhost:4221\r\nUser-Agent: {}\r\nAccept: */*\r\n\r\n", user_agent)).unwrap();
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

        let router = make_router(Some(tmp_dir.path().to_str().unwrap().to_string()));
        let request = HttpRequest::from_string("GET /files/test.txt HTTP/1.1\r\nHost: localhost:4221\r\nUser-Agent: curl/7.64.1\r\nAccept: */*\r\n\r\n").unwrap();
        let response = router.resolve(&request).unwrap();
        assert_eq!(response.status_code, StatusCode::OK);
        assert_eq!(response.body, contents.as_bytes());
        assert_eq!(
            response.to_string().unwrap(),
            format!("HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: 4\r\n\r\n{}", contents)
        );
    }

    #[test]
    fn test_files_file_not_exists() {
        let tmp_dir = TempDir::new("test_files").unwrap();
        let router = make_router(Some(tmp_dir.path().to_str().unwrap().to_string()));
        let request = HttpRequest::from_string("GET /files/test.txt HTTP/1.1\r\nHost: localhost:4221\r\nUser-Agent: curl/7.64.1\r\nAccept: */*\r\n\r\n").unwrap();
        let response = router.resolve(&request).unwrap();
        assert_eq!(response.status_code, StatusCode::NOT_FOUND);
        assert_eq!(response.body, b"");
        assert_eq!(
            response.to_bytes().unwrap(),
            b"HTTP/1.1 404 Not Found\r\n\r\n"
        );
    }

    #[test]
    fn test_files_post() {
        let tmp_dir = TempDir::new("test_files").unwrap();
        let file_path = tmp_dir.path().join("test.txt");
        let router = make_router(Some(tmp_dir.path().to_str().unwrap().to_string()));
        let request = HttpRequest::from_string("POST /files/test.txt HTTP/1.1\r\nHost: localhost:4221\r\nUser-Agent: curl/7.64.1\r\nContent-Length: 4\r\nContent-Type: application/octet-stream\r\nAccept: */*\r\n\r\ntest").unwrap();
        let response = router.resolve(&request).unwrap();
        assert_eq!(response.status_code, StatusCode::CREATED);
        assert_eq!(response.body, b"");
        assert_eq!(
            response.to_bytes().unwrap(),
            b"HTTP/1.1 201 Created\r\n\r\n"
        );
        assert_eq!(std::fs::read_to_string(file_path).unwrap(), "test");
    }

    #[test]
    fn test_post_file_example() {
        let contents = "mango banana mango grape blueberry orange banana grape";
        let filename = "strawberry_blueberry_raspberry_raspberry";
        let tmp_dir = TempDir::new("test_files").unwrap();
        let file_path = tmp_dir.path().join(filename);

        let request_str = format!("POST /files/{} HTTP/1.1\r\nHost: localhost:4221\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\n\r\n{}", filename, contents.len(), contents);
        let request = HttpRequest::from_string(&request_str).unwrap();
        let router = make_router(Some(tmp_dir.path().to_str().unwrap().to_string()));
        let response = router.resolve(&request).unwrap();
        assert_eq!(response.status_code, StatusCode::CREATED);
        assert_eq!(response.body, b"");
        assert_eq!(
            response.to_bytes().unwrap(),
            b"HTTP/1.1 201 Created\r\n\r\n"
        );
        assert_eq!(std::fs::read_to_string(file_path).unwrap(), contents);
    }

    #[test]
    fn test_accept_encoding_gzip() {
        let contents = "abc";
        let mut gzip_encoder =
            flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gzip_encoder.write_all(contents.as_bytes()).unwrap();
        let encoded_contents = gzip_encoder.finish().unwrap();
        let encoded_contents_str = String::from_utf8_lossy(&encoded_contents);

        let request_str = format!("GET /echo/{} HTTP/1.1\r\nHost: localhost:4221\r\nUser-Agent: curl/7.64.1\r\nAccept: gzip\r\n\r\n", contents);
        let router = make_router(None);
        let request = HttpRequest::from_string(&request_str).unwrap();
        let response = router.resolve(&request).unwrap();
        assert_eq!(response.status_code, StatusCode::OK);
        assert_eq!(response.body, contents.as_bytes()); // not encoded

        let expected_response_bytes = [
            b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Encoding: gzip\r\nContent-Length:".to_vec(),
            encoded_contents.len().to_string().as_bytes().to_vec(),
            b"\r\n\r\n".to_vec(),
            encoded_contents
        ].concat();
        assert_eq!(response.to_bytes().unwrap(), expected_response_bytes);
    }
}
