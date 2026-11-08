//! `POST /send`: writes the request body to the serial port.

use std::io::{Read, Write};
use std::sync::Mutex;

pub const MAX_SEND_BYTES: u64 = 1024 * 1024;

const SEND_PATH: &str = "/send";
const SEND_METHOD: &str = "POST";

/// Requiring this type stops web pages posting here: browsers won't send it
/// across sites without a preflight request, which this server never approves.
const REQUIRED_CONTENT_TYPE: &str = "application/octet-stream";

const STATUS_OK: u16 = 200;
const STATUS_BAD_REQUEST: u16 = 400;
const STATUS_NOT_FOUND: u16 = 404;
const STATUS_METHOD_NOT_ALLOWED: u16 = 405;
const STATUS_TOO_LARGE: u16 = 413;
const STATUS_UNSUPPORTED_TYPE: u16 = 415;
const STATUS_UNAVAILABLE: u16 = 503;

#[derive(Debug, PartialEq)]
pub struct Reply {
    pub status: u16,
    pub body: String,
}

fn error(status: u16, message: &str) -> Reply {
    Reply { status, body: format!("{{\"error\":\"{message}\"}}\n") }
}

/// `port` is empty while the serial device is unavailable.
pub fn handle<R: Read, W: Write>(
    method: &str,
    path: &str,
    content_type: Option<&str>,
    body: R,
    port: &Mutex<Option<W>>,
) -> Reply {
    if path != SEND_PATH {
        return error(STATUS_NOT_FOUND, "not found");
    }
    if method != SEND_METHOD {
        return error(STATUS_METHOD_NOT_ALLOWED, "use POST");
    }
    let media_type = content_type.and_then(|value| value.split(';').next()).map(str::trim);
    if !media_type.is_some_and(|value| value.eq_ignore_ascii_case(REQUIRED_CONTENT_TYPE)) {
        return error(STATUS_UNSUPPORTED_TYPE, "Content-Type must be application/octet-stream");
    }

    let mut bytes = Vec::new();
    if body.take(MAX_SEND_BYTES + 1).read_to_end(&mut bytes).is_err() {
        return error(STATUS_BAD_REQUEST, "could not read request body");
    }
    if bytes.len() as u64 > MAX_SEND_BYTES {
        return error(STATUS_TOO_LARGE, "request body is too large");
    }
    if bytes.is_empty() {
        return error(STATUS_BAD_REQUEST, "request body is empty");
    }

    let mut port = port.lock().unwrap();
    let Some(writer) = port.as_mut() else {
        return error(STATUS_UNAVAILABLE, "serial device is unavailable");
    };
    match writer.write_all(&bytes).and_then(|()| writer.flush()) {
        Ok(()) => Reply { status: STATUS_OK, body: format!("{{\"sent\":{}}}\n", bytes.len()) },
        Err(_) => error(STATUS_UNAVAILABLE, "could not write to serial device"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OCTET_STREAM: Option<&str> = Some("application/octet-stream");

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _bytes: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Returns the reply and whatever reached the serial port.
    fn send(method: &str, path: &str, content_type: Option<&str>, body: &[u8]) -> (u16, Vec<u8>) {
        let port = Mutex::new(Some(Vec::new()));
        let reply = handle(method, path, content_type, body, &port);
        (reply.status, port.into_inner().unwrap().unwrap())
    }

    #[test]
    fn a_valid_request_reaches_the_port_and_reports_the_count() {
        let port = Mutex::new(Some(Vec::new()));
        let reply = handle("POST", "/send", OCTET_STREAM, &b"1"[..], &port);
        assert_eq!(reply, Reply { status: 200, body: "{\"sent\":1}\n".to_string() });
        assert_eq!(port.into_inner().unwrap().unwrap(), b"1");
    }

    #[test]
    fn binary_bytes_arrive_unchanged() {
        let body = [0x00, 0x0a, 0x0d, 0x1b, 0x7f, 0x80, 0xfe, 0xff];
        assert_eq!(send("POST", "/send", OCTET_STREAM, &body), (200, body.to_vec()));
    }

    #[test]
    fn content_type_may_vary_in_case_and_carry_parameters() {
        let status = send("POST", "/send", Some("Application/Octet-Stream; x=1"), b"1").0;
        assert_eq!(status, 200);
    }

    #[test]
    fn wrong_path_or_method_sends_nothing() {
        assert_eq!(send("POST", "/other", OCTET_STREAM, b"1"), (404, vec![]));
        assert_eq!(send("GET", "/send", OCTET_STREAM, b"1"), (405, vec![]));
    }

    #[test]
    fn other_content_types_send_nothing() {
        // These are the types a web page can send without a preflight.
        for content_type in ["text/plain", "application/x-www-form-urlencoded", "multipart/form-data"] {
            assert_eq!(send("POST", "/send", Some(content_type), b"1"), (415, vec![]));
        }
        assert_eq!(send("POST", "/send", None, b"1"), (415, vec![]));
    }

    #[test]
    fn an_empty_body_is_rejected() {
        assert_eq!(send("POST", "/send", OCTET_STREAM, b""), (400, vec![]));
    }

    #[test]
    fn the_size_limit_is_exact() {
        let limit = vec![b'x'; MAX_SEND_BYTES as usize];
        assert_eq!(send("POST", "/send", OCTET_STREAM, &limit).0, 200);

        let over = vec![b'x'; MAX_SEND_BYTES as usize + 1];
        assert_eq!(send("POST", "/send", OCTET_STREAM, &over), (413, vec![]));
    }

    #[test]
    fn no_device_reports_unavailable() {
        let port: Mutex<Option<Vec<u8>>> = Mutex::new(None);
        assert_eq!(handle("POST", "/send", OCTET_STREAM, &b"1"[..], &port).status, 503);
    }

    #[test]
    fn a_failed_write_reports_unavailable() {
        let port = Mutex::new(Some(FailingWriter));
        assert_eq!(handle("POST", "/send", OCTET_STREAM, &b"1"[..], &port).status, 503);
    }
}
