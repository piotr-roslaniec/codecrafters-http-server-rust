use crate::http::{HttpRequest, HttpResponse, ResponseHeaders, StatusCode};
use eyre::Result;

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
        Box::new(|_request| HttpResponse::ok(b"", ResponseHeaders::new())),
    );
    let echo_route = Route::new(
        "/echo",
        Box::new(move |request| {
            let path_without_prefix = request.line.path.trim_start_matches("/echo/");
            let mut headers = ResponseHeaders::new();
            headers.insert("Content-Type".to_string(), "text/plain".to_string());
            HttpResponse::ok(path_without_prefix.as_bytes(), headers)
        }),
    );
    let user_agent_route = Route::new(
        "/user-agent",
        Box::new(move |request| {
            let default = String::new();
            let user_agent = request.headers.get("User-Agent").unwrap_or(&default);
            let mut headers = ResponseHeaders::new();
            headers.insert("Content-Type".to_string(), "text/plain".to_string());
            HttpResponse::ok(user_agent.as_bytes(), headers)
        }),
    );
    let files_route = Route::new(
        "/files",
        Box::new(move |request| {
            let headers = {
                let mut headers = ResponseHeaders::new();
                headers.insert(
                    "Content-Type".to_string(),
                    "application/octet-stream".to_string(),
                );
                headers
            };
            let file = request.line.path.trim_start_matches("/files/");
            let binding = String::new();
            let file_path_from_root = directory.as_ref().unwrap_or(&binding);
            let file = format!("{}/{}", file_path_from_root, file);
            let body = match std::fs::read(file) {
                Ok(contents) => contents,
                Err(_) => return HttpResponse::not_found(),
            };
            HttpResponse::new(StatusCode::OK, &body, headers)
        }),
    );

    router.add_route(default_route);
    router.add_route(echo_route);
    router.add_route(user_agent_route);
    router.add_route(files_route);
    router
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::http::StatusCode;
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
        assert_eq!(response.to_bytes(), b"HTTP/1.1 200 OK\r\n\r\n");
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
            response.to_bytes(),
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
        assert_eq!(response.to_bytes(), b"HTTP/1.1 404 Not Found\r\n\r\n");
    }

    #[test]
    fn test_example() {
        let router = make_router(None);
        let request =
            HttpRequest::from_string("GET / HTTP/1.1\r\nHost: localhost:4221\r\n\r\n").unwrap();
        let response = router.resolve(&request).unwrap();
        assert_eq!(response.status_code, StatusCode::OK);
        assert_eq!(response.body, b"");
        assert_eq!(response.to_bytes(), b"HTTP/1.1 200 OK\r\n\r\n");
    }

    #[test]
    fn test_echo_example() {
        let router = make_router(None);
        let request = HttpRequest::from_string("GET /echo/abc HTTP/1.1\r\nHost: localhost:4221\r\nUser-Agent: curl/7.64.1\r\nAccept: */*\r\n\r\n").unwrap();
        let response = router.resolve(&request).unwrap();
        assert_eq!(response.status_code, StatusCode::OK);
        assert_eq!(response.body, b"abc");
        assert_eq!(
            response.to_bytes(),
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
            response.to_bytes(),
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
        assert_eq!(response.to_bytes(), b"HTTP/1.1 404 Not Found\r\n\r\n");
    }
}
