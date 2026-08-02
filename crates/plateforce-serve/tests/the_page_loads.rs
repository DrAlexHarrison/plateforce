//! The page a browser receives, over a real socket, in the type each file has to arrive as.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};

fn a_running_server() -> SocketAddr {
    let listener = plateforce_serve::listen(0).expect("nothing could bind the loopback address");
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || plateforce_serve::serve(listener));
    address
}

fn ask(address: SocketAddr, target: &str, method: &str) -> (String, Vec<u8>) {
    let mut stream = TcpStream::connect(address).unwrap();
    stream
        .write_all(format!("{method} {target} HTTP/1.1\r\nHost: {address}\r\n\r\n").as_bytes())
        .unwrap();
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).unwrap();
    let end_of_head = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("the response carried no header terminator");
    (
        String::from_utf8_lossy(&raw[..end_of_head]).into_owned(),
        raw[end_of_head + 4..].to_vec(),
    )
}

#[test]
fn the_root_is_the_document() {
    let address = a_running_server();
    let (head, body) = ask(address, "/", "GET");

    assert!(head.starts_with("HTTP/1.1 200 OK"), "{head}");
    assert!(
        head.contains("Content-Type: text/html; charset=utf-8"),
        "{head}"
    );
    assert!(String::from_utf8_lossy(&body).contains("<title>plateforce</title>"));
    println!(
        "GET / carried text/html; charset=utf-8, {} bytes",
        body.len()
    );
}

/// The document loads `app.js` as a module and that module imports three more. A browser
/// enforces the type on a module script strictly and refuses to execute one served as
/// anything else, so the page would be blank rather than broken.
#[test]
fn a_module_script_arrives_as_javascript() {
    let address = a_running_server();
    let (head, body) = ask(address, "/app.js", "GET");

    assert!(head.starts_with("HTTP/1.1 200 OK"), "{head}");
    assert!(
        head.contains("Content-Type: text/javascript; charset=utf-8"),
        "{head}"
    );
    assert_eq!(
        body.len(),
        std::fs::metadata(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/app.js"))
            .unwrap()
            .len() as usize
    );
    println!("GET /app.js carried text/javascript; charset=utf-8");
}

/// Served as anything but `application/wasm`, the generated binding falls back from
/// `WebAssembly.instantiateStreaming` to the slower path and records it in a console
/// warning nobody is reading. A silent degradation is the shape of defect this product
/// exists to make visible, and here it is ours to avoid.
#[test]
fn the_module_arrives_as_webassembly() {
    if plateforce_serve::carries_the_browser_bundle() {
        let address = a_running_server();
        let (head, body) = ask(address, "/pkg/plateforce_wasm_bg.wasm", "GET");

        assert!(head.starts_with("HTTP/1.1 200 OK"), "{head}");
        assert!(head.contains("Content-Type: application/wasm"), "{head}");
        assert_eq!(&body[..4], b"\0asm");
        println!("GET /pkg/plateforce_wasm_bg.wasm carried application/wasm over the wire");
    } else {
        assert_eq!(
            plateforce_serve::content_type_for("pkg/plateforce_wasm_bg.wasm"),
            Some("application/wasm")
        );
        println!(
            "web/pkg absent, so application/wasm was checked against the table it is sent from"
        );
    }
}
