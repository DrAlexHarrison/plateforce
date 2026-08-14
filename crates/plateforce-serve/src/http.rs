//! HTTP/1.1 over `std::net`, answering `GET` and `HEAD` from the compiled-in table.

use std::io::{BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::time::Duration;

use crate::assets::asset_for;

/// A request line and its headers cannot exceed this. The client is a browser on the same
/// machine asking for an embedded file, which is well under a kilobyte.
const LARGEST_REQUEST_HEAD_BYTES: usize = 8 * 1024;

/// A connection that opens and sends nothing must not hold the port. Browsers open sockets
/// ahead of the requests they may put on them.
const HOW_LONG_A_CONNECTION_MAY_STAY_SILENT: Duration = Duration::from_secs(10);
const HOW_LONG_A_RESPONSE_MAY_TAKE: Duration = Duration::from_secs(30);

/// `connect-src 'self'` is what makes the claim that nothing leaves the machine something
/// the browser enforces rather than something this code promises. `wasm-unsafe-eval` is
/// what a WebAssembly module needs to instantiate at all, and `img-src data:` is the
/// document's own inline mark, which the fallback to `default-src` blocks: measured in
/// Chrome, which dropped the favicon and wrote the reason to a console nobody reads.
const CONTENT_SECURITY_POLICY: &str = "default-src 'self'; \
     script-src 'self' 'wasm-unsafe-eval'; \
     img-src 'self' data:; \
     connect-src 'self'";

const PLAIN_TEXT: &str = "text/plain; charset=utf-8";

pub(crate) struct Request {
    pub method: String,
    pub target: String,
}

#[derive(Debug)]
pub(crate) enum RequestProblem {
    HeadTooLarge,
    Unreadable,
}

pub(crate) struct Response {
    pub status: &'static str,
    pub content_type: &'static str,
    pub body: &'static [u8],
    pub allow: Option<&'static str>,
    pub send_body: bool,
}

/// Binds the loopback interface and nothing else. Port 0 asks the operating system for a
/// free port, which the caller then reads back and prints.
pub fn listen(port: u16) -> std::io::Result<TcpListener> {
    TcpListener::bind((Ipv4Addr::LOCALHOST, port))
}

/// Answers connections until the process is stopped. Each connection gets its own thread,
/// so a browser that opens a socket and puts no request on it delays nothing else.
pub fn serve(listener: TcpListener) {
    for connection in listener.incoming() {
        match connection {
            Ok(stream) => {
                std::thread::spawn(move || answer_one_connection(stream));
            }
            // Silence about a refused connection would leave an operator watching a page
            // that never loads with nothing to read.
            Err(error) => eprintln!("plateforce: a connection could not be accepted: {error}"),
        }
    }
}

fn answer_one_connection(stream: TcpStream) {
    let _ = stream.set_read_timeout(Some(HOW_LONG_A_CONNECTION_MAY_STAY_SILENT));
    let _ = stream.set_write_timeout(Some(HOW_LONG_A_RESPONSE_MAY_TAKE));

    let response = match read_request(BufReader::new(&stream)) {
        Ok(request) => answer(&request),
        Err(RequestProblem::HeadTooLarge) => Response {
            status: "431 Request Header Fields Too Large",
            content_type: PLAIN_TEXT,
            body: b"That request is larger than this server reads.\n",
            allow: None,
            send_body: true,
        },
        Err(RequestProblem::Unreadable) => Response {
            status: "400 Bad Request",
            content_type: PLAIN_TEXT,
            body: b"This server did not recognise that request.\n",
            allow: None,
            send_body: true,
        },
    };

    let mut stream = stream;
    let _ = write_response(&mut stream, &response);
    let _ = stream.flush();
}

pub(crate) fn read_request(source: impl Read) -> Result<Request, RequestProblem> {
    let mut source = source;
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match source.read(&mut byte) {
            Ok(0) => return Err(RequestProblem::Unreadable),
            Ok(_) => head.push(byte[0]),
            Err(_) => return Err(RequestProblem::Unreadable),
        }
        if head.len() > LARGEST_REQUEST_HEAD_BYTES {
            return Err(RequestProblem::HeadTooLarge);
        }
        if head.ends_with(b"\r\n\r\n") {
            break;
        }
    }

    let head = String::from_utf8(head).map_err(|_| RequestProblem::Unreadable)?;
    let first_line = head.lines().next().ok_or(RequestProblem::Unreadable)?;
    let mut words = first_line.split(' ');
    let (Some(method), Some(target), Some(version), None) =
        (words.next(), words.next(), words.next(), words.next())
    else {
        return Err(RequestProblem::Unreadable);
    };
    if !version.starts_with("HTTP/") || method.is_empty() || target.is_empty() {
        return Err(RequestProblem::Unreadable);
    }

    Ok(Request {
        method: method.to_string(),
        target: target.to_string(),
    })
}

pub(crate) fn answer(request: &Request) -> Response {
    let send_body = match request.method.as_str() {
        "GET" => true,
        "HEAD" => false,
        _ => {
            return Response {
                status: "405 Method Not Allowed",
                content_type: PLAIN_TEXT,
                body: b"This server answers GET and HEAD.\n",
                allow: Some("GET, HEAD"),
                send_body: true,
            }
        }
    };

    match asset_for(&request.target) {
        Some(asset) => Response {
            status: "200 OK",
            content_type: asset.content_type,
            body: asset.bytes,
            allow: None,
            send_body,
        },
        None => Response {
            status: "404 Not Found",
            content_type: PLAIN_TEXT,
            body: b"This address is not part of the plateforce interface, which is at /.\n",
            allow: None,
            send_body,
        },
    }
}

/// `no-store` because the whole interface is inside the binary, so an operator who upgrades
/// and restarts must not be handed a cached copy of the module they replaced.
fn write_response(sink: &mut impl Write, response: &Response) -> std::io::Result<()> {
    let mut head = format!(
        "HTTP/1.1 {}\r\n\
         Content-Type: {}\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\n\
         Content-Security-Policy: {}\r\n\
         Connection: close\r\n",
        response.status,
        response.content_type,
        response.body.len(),
        CONTENT_SECURITY_POLICY,
    );
    if let Some(allow) = response.allow {
        head.push_str(&format!("Allow: {allow}\r\n"));
    }
    head.push_str("\r\n");

    sink.write_all(head.as_bytes())?;
    if response.send_body {
        sink.write_all(response.body)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(head: &str) -> Result<Request, RequestProblem> {
        read_request(head.as_bytes())
    }

    #[test]
    fn a_request_line_and_its_headers_are_read() {
        let parsed = request("GET /app.js HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").unwrap();
        assert_eq!(parsed.method, "GET");
        assert_eq!(parsed.target, "/app.js");
    }

    /// A browser opening a socket ahead of a request, and a client sending something that
    /// is not HTTP, both end the connection rather than being read as a request for the
    /// document.
    #[test]
    fn what_is_not_a_request_is_not_read_as_one() {
        assert!(matches!(
            request("\r\n\r\n"),
            Err(RequestProblem::Unreadable)
        ));
        assert!(matches!(
            request("GET /\r\n\r\n"),
            Err(RequestProblem::Unreadable)
        ));
        assert!(matches!(
            request("GET / SOMETHING/1.1\r\n\r\n"),
            Err(RequestProblem::Unreadable)
        ));
    }

    /// The cap is the reason a request cannot grow this process's memory.
    #[test]
    fn a_request_head_larger_than_the_cap_is_refused() {
        let padding = "x".repeat(LARGEST_REQUEST_HEAD_BYTES + 1);
        let oversize = format!("GET / HTTP/1.1\r\nHost: {padding}\r\n\r\n");
        assert!(matches!(
            request(&oversize),
            Err(RequestProblem::HeadTooLarge)
        ));
    }

    #[test]
    fn head_carries_the_headers_and_no_body() {
        let answered = answer(&Request {
            method: "HEAD".to_string(),
            target: "/".to_string(),
        });
        assert_eq!(answered.status, "200 OK");
        assert!(!answered.send_body);
        assert!(!answered.body.is_empty());
    }

    #[test]
    fn another_method_is_refused_with_what_this_server_answers() {
        let answered = answer(&Request {
            method: "POST".to_string(),
            target: "/".to_string(),
        });
        assert_eq!(answered.status, "405 Method Not Allowed");
        assert_eq!(answered.allow, Some("GET, HEAD"));
    }

    #[test]
    fn every_response_carries_the_policy_that_keeps_the_page_local() {
        let mut written = Vec::new();
        write_response(
            &mut written,
            &answer(&Request {
                method: "GET".to_string(),
                target: "/".to_string(),
            }),
        )
        .unwrap();
        let head = String::from_utf8_lossy(&written);
        assert!(head.contains("connect-src 'self'"));
        assert!(head.contains("wasm-unsafe-eval"));
        assert!(head.contains("Cache-Control: no-store"));
        assert!(head.contains("X-Content-Type-Options: nosniff"));
    }

    /// A policy that blocks something the document itself carries breaks the page silently:
    /// the browser drops the request and writes the reason to a console nobody is reading.
    /// Read off the document rather than written down here, so a mark added or removed
    /// later moves this test with it.
    #[test]
    fn the_policy_admits_what_the_document_carries_inline() {
        let document = crate::asset_for("/").expect("no document is embedded");
        if String::from_utf8_lossy(document.bytes).contains("data:image") {
            assert!(
                CONTENT_SECURITY_POLICY.contains("img-src")
                    && CONTENT_SECURITY_POLICY.contains("data:"),
                "the document carries an inline image and the policy blocks it"
            );
        }
    }
}
