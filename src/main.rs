use std::io::Write;
use std::net::TcpListener;

fn main() {
    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream_) => {
                println!("accepted new connection");
                let buffer = "HTTP/1.1 200 OK\r\n\r\n";
                stream_.write_all(buffer.as_ref()).unwrap()
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
