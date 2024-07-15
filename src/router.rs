use crate::error::Result;
use crate::http::{HttpRequest, HttpResponse};

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

    pub fn resolve(&self, request: HttpRequest) -> Result<HttpResponse> {
        for route in &self.routes {
            if self.parse_path(&request.request_line.path) == self.parse_path(&route.path) {
                return Ok((route.handler)(request));
            }
        }
        Ok(HttpResponse::not_found())
    }
}

pub struct Route {
    path: String,
    handler: fn(HttpRequest) -> HttpResponse,
}

impl Route {
    pub fn new(path: &str, handler: fn(HttpRequest) -> HttpResponse) -> Self {
        Self {
            path: path.to_string(),
            handler,
        }
    }
}

pub fn make_router() -> Router {
    let mut router = Router::new();

    let default_route = Route::new("/", |_request| HttpResponse::ok(b""));
    let echo_route = Route::new("/echo", |request| {
        let path_without_prefix = request.request_line.path.trim_start_matches("/echo/");
        HttpResponse::ok(path_without_prefix.as_bytes())
    });

    router.add_route(default_route);
    router.add_route(echo_route);
    router
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::http::StatusCode;

    #[test]
    fn test_router_parse_path() {
        let router = make_router();
        assert_eq!(router.parse_path("/"), "");
        assert_eq!(router.parse_path("/echo"), "echo");
        assert_eq!(router.parse_path("/echo/"), "echo");
        assert_eq!(router.parse_path("/echo/123"), "echo");
    }

    #[test]
    fn test_router_resolve_root() {
        let router = make_router();
        let request = HttpRequest::from_string("GET / HTTP/1.1\r\nHost: localhost:4221\r\nUser-Agent: curl/7.64.1\r\nAccept: */*\r\n\r\n").unwrap();
        let response = router.resolve(request).unwrap();
        assert_eq!(response.status_code, StatusCode::OK);
        assert_eq!(response.body, b"");
        assert_eq!(
            response.to_bytes(),
            b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 0\r\n\r\n"
        );
    }

    #[test]
    fn test_router_resolve_echo() {
        let expected_body = "my_test_path";
        let router = make_router();
        let request = HttpRequest::from_string(&format!("GET /echo/{} HTTP/1.1\r\nHost: localhost:4221\r\nUser-Agent: curl/7.64.1\r\nAccept: */*\r\n\r\n", expected_body)).unwrap();
        let response = router.resolve(request).unwrap();
        assert_eq!(response.status_code, StatusCode::OK);
        assert_eq!(response.body, expected_body.as_bytes());
        assert_eq!(
            response.to_bytes(),
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n{}\r\n\r\n",
                expected_body.len(),
                expected_body
            )
            .as_bytes()
        );
    }

    #[test]
    fn test_router_resolve_not_found() {
        let router = make_router();
        let request = HttpRequest::from_string("GET /not_found HTTP/1.1\r\nHost: localhost:4221\r\nUser-Agent: curl/7.64.1\r\nAccept: */*\r\n\r\n").unwrap();
        let response = router.resolve(request).unwrap();
        assert_eq!(response.status_code, StatusCode::NOT_FOUND);
        assert_eq!(response.body, b"Not Found");
        assert_eq!(response.to_bytes(), b"HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\nContent-Length: 9\r\nNot Found\r\n\r\n");
    }

    #[test]
    fn test_example() {
        let router = make_router();
        let request = HttpRequest::from_string("GET /echo/abc HTTP/1.1\r\nHost: localhost:4221\r\nUser-Agent: curl/7.64.1\r\nAccept: */*\r\n\r\n").unwrap();
        let response = router.resolve(request).unwrap();
        assert_eq!(response.status_code, StatusCode::OK);
        assert_eq!(response.body, b"abc");
        assert_eq!(
            response.to_bytes(),
            b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 3\r\nabc\r\n\r\n"
        );
    }
}
