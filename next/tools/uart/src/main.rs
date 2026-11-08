//! Displays a serial port's output by connecting to `cargo server`.

use std::io::{self, BufRead, BufReader, ErrorKind, Read, Write};
use std::net::TcpStream;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use protocol::{decode_packet, MAX_PACKET_BYTES};

const DEFAULT_SERVER: &str = "localhost:8765";
const BUFFER_SIZE: usize = 4096;
const BOUNDARY: u8 = 0;

/// Must match the server's `HEADER_PREFIX`.
const HEADER_PREFIX: &str = "kitsune-uart history=";
const HEADER_TIMEOUT: Duration = Duration::from_secs(5);

/// How long a read waits for data, so an idle connection still notices the time limit.
const READ_TIMEOUT: Duration = Duration::from_millis(100);

const USAGE: &str = "\
Displays serial output from `cargo server`. Non-printable bytes appear as \\xNN.
Shows recent history first, then live output.

Usage: cargo uart [OPTIONS]

Options:
  --server <ADDRESS>  Server to connect to [default: localhost:8765]
  --seconds <N>       Stop after N seconds and print a summary [default: run until Ctrl-C]
  --live              Skip the history and show only new output
  --help              Show this message";

#[derive(Debug, PartialEq)]
struct Options {
    server: String,
    seconds: Option<u64>,
    live: bool,
}

/// Returns `None` if the user asked for help.
fn parse_options(args: impl IntoIterator<Item = String>) -> Result<Option<Options>, String> {
    let mut options = Options { server: DEFAULT_SERVER.to_string(), seconds: None, live: false };
    let mut args = args.into_iter();

    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--help" => return Ok(None),
            "--live" => options.live = true,
            "--server" => options.server = args.next().ok_or("--server needs a value")?,
            "--seconds" => {
                let value = args.next().ok_or("--seconds needs a value")?;
                let seconds = value.parse().map_err(|_| format!("--seconds needs a number, got '{value}'"))?;
                options.seconds = Some(seconds);
            }
            _ => return Err(format!("unknown option: {flag}")),
        }
    }
    Ok(Some(options))
}

fn parse_header(line: &str) -> Result<u64, String> {
    line.trim_end()
        .strip_prefix(HEADER_PREFIX)
        .and_then(|length| length.parse().ok())
        .ok_or(format!("unexpected response from server: {line:?}"))
}

/// Printable text, tabs and line endings pass through. Everything else is
/// escaped, so binary data can't confuse the terminal and stays readable.
fn render(bytes: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(bytes.len());
    for &byte in bytes {
        match byte {
            b'\t' | b'\n' | b'\r' | 0x20..=0x7E => output.push(byte),
            _ => output.extend_from_slice(format!("\\x{byte:02x}").as_bytes()),
        }
    }
    output
}

fn connect(options: &Options) -> Result<(BufReader<TcpStream>, u64), String> {
    let server = &options.server;
    let stream = TcpStream::connect(server)
        .map_err(|error| format!("could not connect to {server} (is `cargo server` running?): {error}"))?;

    stream.set_read_timeout(Some(HEADER_TIMEOUT)).map_err(|error| error.to_string())?;
    let mut reader = BufReader::new(stream);
    let mut header = String::new();
    reader.read_line(&mut header).map_err(|error| format!("could not read from {server}: {error}"))?;
    let history_length = parse_header(&header)?;

    reader.get_ref().set_read_timeout(Some(READ_TIMEOUT)).map_err(|error| error.to_string())?;
    Ok((reader, history_length))
}

/// Writes to stdout. Returns false once whatever is reading has gone away, as with `| head`.
fn display(bytes: &[u8]) -> Result<bool, String> {
    let mut stdout = io::stdout().lock();
    match stdout.write_all(&render(bytes)).and_then(|()| stdout.flush()) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == ErrorKind::BrokenPipe => Ok(false),
        Err(error) => Err(format!("could not write output: {error}")),
    }
}

/// Takes the kernel's packets out of its output, leaving the console text. What's between
/// two zeros is a packet if its CRC is right. Anything else is text, including a stray
/// zero, as a power change sends, and what follows it.
#[derive(Default)]
struct PacketFilter {
    /// What's been seen since a zero that might have started a packet.
    pending: Option<Vec<u8>>,
}

impl PacketFilter {
    fn text_from(&mut self, bytes: &[u8]) -> Vec<u8> {
        let mut text = Vec::with_capacity(bytes.len());
        for byte in bytes {
            match (self.pending.as_mut(), *byte) {
                (None, BOUNDARY) => self.pending = Some(Vec::new()),
                (None, other) => text.push(other),
                (Some(pending), BOUNDARY) => {
                    if !pending.is_empty() && decode_packet(pending).is_some() {
                        self.pending = None;
                    } else {
                        let not_a_packet = core::mem::take(pending);
                        text.push(BOUNDARY);
                        text.extend(not_a_packet);
                    }
                }
                (Some(pending), other) => {
                    pending.push(other);
                    // Longer than any packet, so it's text, which shouldn't wait any longer.
                    if pending.len() > MAX_PACKET_BYTES {
                        text.extend(self.flush());
                    }
                }
            }
        }
        text
    }

    /// Hands over anything being held back, for when the line has gone quiet.
    fn flush(&mut self) -> Vec<u8> {
        let Some(pending) = self.pending.take() else {
            return Vec::new();
        };
        let mut text = vec![BOUNDARY];
        text.extend(pending);
        text
    }
}

/// Displays output until the time limit, returning how many new bytes arrived.
fn stream(options: &Options, reader: &mut BufReader<TcpStream>, history_length: u64) -> Result<u64, String> {
    let limit = options.seconds.map(Duration::from_secs);
    let start = Instant::now();
    let mut buffer = [0; BUFFER_SIZE];
    let mut history_left = history_length;
    let mut new_bytes = 0;
    let mut filter = PacketFilter::default();

    while limit.is_none_or(|limit| start.elapsed() < limit) {
        let count = match reader.read(&mut buffer) {
            Ok(0) => return Err(format!("{} closed the connection", options.server)),
            Ok(count) => count,
            Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted) => {
                // The line has gone quiet, so what's held back isn't the start of a packet.
                let text = filter.flush();
                if !text.is_empty() && !display(&text)? {
                    return Ok(new_bytes);
                }
                continue;
            }
            Err(error) => return Err(format!("could not read from {}: {error}", options.server)),
        };

        let history_part = count.min(history_left as usize);
        let (old, new) = buffer[..count].split_at(history_part);
        history_left -= history_part as u64;
        new_bytes += new.len() as u64;

        let shown = if options.live { &[][..] } else { old };
        let text = filter.text_from(&[shown, new].concat());
        if !text.is_empty() && !display(&text)? {
            return Ok(new_bytes);
        }
    }
    Ok(new_bytes)
}

fn run(options: &Options) -> Result<(), String> {
    let (mut reader, history_length) = connect(options)?;

    let until = match options.seconds {
        Some(seconds) => format!("for {seconds}s"),
        None => "until Ctrl-C".to_string(),
    };
    let history = if options.live { "skipping" } else { "showing" };
    eprintln!("uart: connected to {} {until}, {history} {history_length} bytes of history", options.server);

    let new_bytes = stream(options, &mut reader, history_length)?;
    eprintln!("\nuart: received {new_bytes} new bytes");
    Ok(())
}

fn main() -> ExitCode {
    let options = match parse_options(std::env::args().skip(1)) {
        Ok(Some(options)) => options,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("uart: {message}\n\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };

    match run(&options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("uart: {message}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::{Message, UploadStatus};

    fn parse(args: &[&str]) -> Result<Option<Options>, String> {
        parse_options(args.iter().map(|arg| arg.to_string()))
    }

    fn filtered(chunks: &[&[u8]]) -> Vec<u8> {
        let mut filter = PacketFilter::default();
        let mut text = Vec::new();
        for chunk in chunks {
            text.extend(filter.text_from(chunk));
        }
        text.extend(filter.flush());
        text
    }

    fn reply_packet() -> Vec<u8> {
        let mut packet = [0; MAX_PACKET_BYTES];
        let message = Message::UploadReply { next_offset: 480, status: UploadStatus::Receiving };
        let length = message.encode(&mut packet);
        packet[..length].to_vec()
    }

    #[test]
    fn packets_are_taken_out_of_the_text() {
        let packet = reply_packet();
        assert_eq!(filtered(&[b"ab", &packet, b"cd", &packet, &packet, b"e"]), b"abcde");
    }

    #[test]
    fn a_packet_split_across_reads_is_still_recognised() {
        let packet = reply_packet();
        let (first, second) = packet.split_at(3);
        assert_eq!(filtered(&[b"a", first, second, b"b"]), b"ab");
    }

    #[test]
    fn a_stray_zero_keeps_its_text() {
        assert_eq!(filtered(&[b"a\0hello"]), b"a\0hello");
    }

    #[test]
    fn two_stray_zeros_keep_the_text_between_them() {
        assert_eq!(filtered(&[b"\0one\0two"]), b"\0one\0two");
    }

    #[test]
    fn a_packet_after_a_stray_zero_and_text_is_still_found() {
        let packet = reply_packet();
        assert_eq!(filtered(&[b"\0text", &packet, b"after"]), b"\0textafter");
    }

    #[test]
    fn a_lot_of_text_after_a_stray_zero_comes_through_without_waiting() {
        let long = vec![b'x'; MAX_PACKET_BYTES * 2];
        let mut filter = PacketFilter::default();
        filter.text_from(b"\0");
        assert!(filter.text_from(&long).len() >= MAX_PACKET_BYTES);
    }

    #[test]
    fn render_passes_ordinary_text_through() {
        assert_eq!(render(b"Hello, World!\r\n\tok ~"), b"Hello, World!\r\n\tok ~");
    }

    #[test]
    fn render_escapes_everything_else() {
        assert_eq!(render(&[0x00, 0x1b, 0x7f, 0x80, 0xff]), b"\\x00\\x1b\\x7f\\x80\\xff");
    }

    #[test]
    fn no_options_gives_the_defaults() {
        // Literal values: comparing against the constants would pass whatever they held.
        let expected = Options { server: "localhost:8765".to_string(), seconds: None, live: false };
        assert_eq!(parse(&[]), Ok(Some(expected)));
    }

    #[test]
    fn options_override_the_defaults() {
        let expected = Options { server: "pi:1".to_string(), seconds: Some(3), live: true };
        assert_eq!(parse(&["--server", "pi:1", "--seconds", "3", "--live"]), Ok(Some(expected)));
    }

    #[test]
    fn help_returns_none() {
        assert_eq!(parse(&["--help"]), Ok(None));
    }

    #[test]
    fn bad_input_is_rejected() {
        assert!(parse(&["--nope"]).is_err());
        assert!(parse(&["--seconds"]).is_err());
        assert!(parse(&["--seconds", "soon"]).is_err());
    }

    #[test]
    fn header_gives_the_history_length() {
        assert_eq!(parse_header("kitsune-uart history=42\n"), Ok(42));
    }

    #[test]
    fn a_bad_header_is_rejected() {
        assert!(parse_header("").is_err());
        assert!(parse_header("HTTP/1.1 200 OK\n").is_err());
        assert!(parse_header("kitsune-uart history=lots\n").is_err());
    }
}
