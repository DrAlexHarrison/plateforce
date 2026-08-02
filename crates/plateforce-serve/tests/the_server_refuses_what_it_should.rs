//! What the server declines, and what it tells the reader instead.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};

fn a_running_server() -> SocketAddr {
    let listener = plateforce_serve::listen(0).expect("nothing could bind the loopback address");
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || plateforce_serve::serve(listener));
    address
}

fn ask(address: SocketAddr, target: &str, method: &str) -> String {
    let mut stream = TcpStream::connect(address).unwrap();
    stream
        .write_all(format!("{method} {target} HTTP/1.1\r\nHost: {address}\r\n\r\n").as_bytes())
        .unwrap();
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).unwrap();
    String::from_utf8_lossy(&raw).into_owned()
}

/// A path outside the compiled-in table, including one that tries to leave it. No
/// filesystem path is ever built from a request, so the traversal reads as an unknown
/// name rather than as an escape.
#[test]
fn a_path_the_interface_does_not_have_is_named_rather_than_guessed() {
    let address = a_running_server();

    for target in ["/nowhere.js", "/../Cargo.toml", "/pkg/../../etc/passwd"] {
        let response = ask(address, target, "GET");
        assert!(
            response.starts_with("HTTP/1.1 404 Not Found"),
            "{target} answered {response}"
        );
        assert!(response.contains("not part of the plateforce interface"));
    }
    println!("404 on three targets outside the table, including two traversals");
}

#[test]
fn another_method_is_refused_with_the_ones_this_server_answers() {
    let address = a_running_server();

    for method in ["POST", "PUT", "DELETE"] {
        let response = ask(address, "/", method);
        assert!(
            response.starts_with("HTTP/1.1 405 Method Not Allowed"),
            "{method} answered {response}"
        );
        assert!(response.contains("Allow: GET, HEAD"), "{response}");
    }
    println!("405 with Allow: GET, HEAD on three other methods");
}
