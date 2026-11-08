//! `POST /mouse` and `POST /key`: turn what a browser sends into packets for the kernel.
//!
//! The page sends plain numbers, so that it doesn't need to know about packets. There's
//! no log line per request, as mouse moves come about 60 times a second.

use std::io::{Read, Write};
use std::sync::Mutex;

use protocol::{Message, MAX_PACKET_BYTES};

const MOUSE_PATH: &str = "/mouse";
const KEY_PATH: &str = "/key";
/// Two little-endian `i16`s, `dx` and `dy`.
const MOUSE_BODY_BYTES: usize = 4;
const STATUS_OK: u16 = 200;
const STATUS_BAD_REQUEST: u16 = 400;
const STATUS_METHOD_NOT_ALLOWED: u16 = 405;
const STATUS_UNAVAILABLE: u16 = 503;

/// Reads a body that must be exactly this long.
fn read_exactly<const LENGTH: usize, R: Read>(mut body: R) -> Option<[u8; LENGTH]> {
    let mut bytes = [0; LENGTH];
    let mut extra = [0; 1];
    let good = body.read_exact(&mut bytes).is_ok() && matches!(body.read(&mut extra), Ok(0));
    good.then_some(bytes)
}

fn write_packet<W: Write>(message: Message, port: &Mutex<Option<W>>) -> u16 {
    let mut packet = [0; MAX_PACKET_BYTES];
    let length = message.encode(&mut packet);
    let mut port = port.lock().expect("a thread panicked while writing to the port");
    match port.as_mut().map(|writer| writer.write_all(&packet[..length]).and_then(|()| writer.flush())) {
        Some(Ok(())) => STATUS_OK,
        _ => STATUS_UNAVAILABLE,
    }
}

/// Handles a request if it's for one of these paths, returning its status. `port` is
/// empty while the serial device is unavailable.
pub fn handle<R: Read, W: Write>(method: &str, path: &str, body: R, port: &Mutex<Option<W>>) -> Option<u16> {
    if path != MOUSE_PATH && path != KEY_PATH {
        return None;
    }
    if method != "POST" {
        return Some(STATUS_METHOD_NOT_ALLOWED);
    }
    let message = if path == MOUSE_PATH {
        read_exactly::<MOUSE_BODY_BYTES, _>(body).map(|bytes| Message::MouseMove {
            dx: i16::from_le_bytes([bytes[0], bytes[1]]),
            dy: i16::from_le_bytes([bytes[2], bytes[3]]),
        })
    } else {
        read_exactly::<1, _>(body).map(|[key]| Message::Key(key))
    };
    Some(message.map_or(STATUS_BAD_REQUEST, |message| write_packet(message, port)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::{Received, Receiver};

    fn post(path: &str, body: &[u8]) -> (u16, Vec<u8>) {
        let port = Mutex::new(Some(Vec::new()));
        let status = handle("POST", path, body, &port).unwrap();
        (status, port.into_inner().unwrap().unwrap())
    }

    fn received(written: &[u8]) -> Vec<Received> {
        let mut receiver = Receiver::new();
        written.iter().map(|byte| receiver.push(*byte)).filter(|received| *received != Received::Absorbed).collect()
    }

    #[test]
    fn a_move_reaches_the_port_as_a_packet() {
        let (status, written) = post(MOUSE_PATH, &[0x2C, 0x01, 0xFE, 0xFF]);
        assert_eq!(status, STATUS_OK);
        assert_eq!(received(&written), [Received::Message(Message::MouseMove { dx: 300, dy: -2 })]);
    }

    #[test]
    fn a_key_reaches_the_port_as_a_packet() {
        let (status, written) = post(KEY_PATH, b"3");
        assert_eq!(status, STATUS_OK);
        assert_eq!(received(&written), [Received::Message(Message::Key(b'3'))]);
    }

    #[test]
    fn a_body_of_the_wrong_size_writes_nothing() {
        for body in [&[][..], &[1, 2, 3], &[1, 2, 3, 4, 5]] {
            assert_eq!(post(MOUSE_PATH, body), (STATUS_BAD_REQUEST, Vec::new()), "{body:?}");
        }
        for body in [&[][..], b"12"] {
            assert_eq!(post(KEY_PATH, body), (STATUS_BAD_REQUEST, Vec::new()), "{body:?}");
        }
    }

    #[test]
    fn only_post_is_allowed() {
        let port = Mutex::new(Some(Vec::new()));
        assert_eq!(handle("GET", MOUSE_PATH, &[0u8, 0, 0, 0][..], &port), Some(STATUS_METHOD_NOT_ALLOWED));
        assert_eq!(handle("GET", KEY_PATH, &b"1"[..], &port), Some(STATUS_METHOD_NOT_ALLOWED));
    }

    #[test]
    fn no_port_is_reported() {
        let port: Mutex<Option<Vec<u8>>> = Mutex::new(None);
        assert_eq!(handle("POST", KEY_PATH, &b"1"[..], &port), Some(STATUS_UNAVAILABLE));
    }

    #[test]
    fn other_paths_are_left_for_others() {
        let port = Mutex::new(Some(Vec::new()));
        assert_eq!(handle("POST", "/send", &b"1"[..], &port), None);
        assert_eq!(port.into_inner().unwrap().unwrap(), Vec::<u8>::new());
    }
}
