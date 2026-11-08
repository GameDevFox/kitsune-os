//! Sends a file to the card in the Pi, over serial, through `cargo server`.
//!
//! Each chunk waits for the kernel's reply, and gets resent if none comes. The kernel keeps
//! the whole file in RAM until its CRC-32 checks out, and only then writes to the card.
use std::env;
use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use protocol::{crc32, decode_packet, Message, Target, UploadStatus, CHUNK_BYTES, MAX_PACKET_BYTES};

const VIEWER_ADDRESS: &str = "127.0.0.1:8765";
const SEND_ADDRESS: &str = "127.0.0.1:8766";
const DEFAULT_FILE: &str = "target/kernel-only.img";
const REPLY_TIMEOUT: Duration = Duration::from_millis(500);
/// Writing and reading back 512 KB takes about 4 seconds.
const COMMIT_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RETRIES: u32 = 8;
const READ_POLL: Duration = Duration::from_millis(20);
const BOUNDARY: u8 = 0;

const USAGE: &str = "\
Usage: cargo upload scratch [FILE] [--corrupt-every N] [--drop-every N] [--wrong-crc]
       cargo upload trial [FILE]
       cargo upload kernel [FILE] --really-replace-the-boot-kernel
       cargo upload reboot [--trial]
       Add --from-memory to say the file is already in the Pi's upload memory, so that only
       its checks are sent. `cargo pi --install` does this.

Sends FILE, by default target/kernel-only.img, to the card in the Pi: update-test.bin for
`scratch`, kitsune-try.img for `trial`, or kitsune-next.img, which the Pi boots from, for
`kernel`. A kernel that passes its checksum might still not boot, and then recovering needs
the card in a PC, so try it first: `reboot --trial` boots kitsune-try.img once, and any later
reboot goes back to kitsune-next.img.

The other options are for testing: damage or leave out every Nth chunk, or send the wrong CRC.";

#[derive(Debug, Default, PartialEq, Eq)]
struct Faults {
    corrupt_every: Option<usize>,
    drop_every: Option<usize>,
    wrong_crc: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct Options {
    target: Target,
    file: String,
    faults: Faults,
    from_memory: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Upload(Options),
    Reboot { trial: bool },
}

fn parse_options(arguments: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let mut arguments = arguments.into_iter();
    let target = match arguments.next().as_deref() {
        Some("scratch") => Target::Scratch,
        Some("kernel") => Target::Kernel,
        Some("trial") => Target::Trial,
        Some("reboot") => {
            return match arguments.next().as_deref() {
                None => Ok(Command::Reboot { trial: false }),
                Some("--trial") if arguments.next().is_none() => Ok(Command::Reboot { trial: true }),
                Some(other) => Err(format!("unexpected {other}")),
            };
        }
        Some(other) => return Err(format!("unknown target {other}")),
        None => return Err("no target given".to_string()),
    };
    let mut file = None;
    let mut faults = Faults::default();
    let mut confirmed = false;
    let mut from_memory = false;
    while let Some(argument) = arguments.next() {
        let mut every = || arguments.next().and_then(|value| value.parse().ok()).filter(|every| *every > 0).ok_or(format!("{argument} needs a number"));
        match argument.as_str() {
            "--corrupt-every" => faults.corrupt_every = Some(every()?),
            "--drop-every" => faults.drop_every = Some(every()?),
            "--wrong-crc" => faults.wrong_crc = true,
            "--from-memory" => from_memory = true,
            "--really-replace-the-boot-kernel" => confirmed = true,
            option if option.starts_with('-') => return Err(format!("unknown option {option}")),
            _ if file.is_none() => file = Some(argument),
            _ => return Err(format!("unexpected {argument}")),
        }
    }
    if target == Target::Kernel && !confirmed {
        return Err("replacing the boot kernel needs --really-replace-the-boot-kernel".to_string());
    }
    if target != Target::Scratch && faults != Faults::default() {
        return Err("test faults are only for the scratch file".to_string());
    }
    if from_memory && (faults.corrupt_every.is_some() || faults.drop_every.is_some()) {
        return Err("nothing's sent to damage with --from-memory".to_string());
    }
    Ok(Command::Upload(Options { target, file: file.unwrap_or_else(|| DEFAULT_FILE.to_string()), faults, from_memory }))
}

/// Pulls packets out of the kernel's output, which is mostly console text. What's between
/// two zeros is a packet if its CRC is right, and text otherwise.
#[derive(Default)]
struct Packets {
    pending: Vec<u8>,
}

impl Packets {
    fn push(&mut self, byte: u8) -> Option<Message> {
        if byte != BOUNDARY {
            self.pending.push(byte);
            return None;
        }
        let message = decode_packet(&self.pending);
        self.pending.clear();
        message
    }
}

struct Viewer {
    stream: TcpStream,
    packets: Packets,
}

impl Viewer {
    /// Connects, and skips the history that the server starts with.
    fn connect() -> Result<Self, String> {
        let mut stream = TcpStream::connect(VIEWER_ADDRESS).map_err(|error| format!("can't reach cargo server: {error}"))?;
        let mut header = Vec::new();
        let mut byte = [0; 1];
        while byte != [b'\n'] {
            stream.read_exact(&mut byte).map_err(|error| error.to_string())?;
            header.push(byte[0]);
        }
        let text = String::from_utf8_lossy(&header);
        let mut left: usize = text.trim().rsplit('=').next().and_then(|value| value.parse().ok()).ok_or("the server's header is unexpected")?;
        let mut discard = [0; 4096];
        while left > 0 {
            let count = stream.read(&mut discard[..left.min(4096)]).map_err(|error| error.to_string())?;
            left -= count;
        }
        stream.set_read_timeout(Some(READ_POLL)).map_err(|error| error.to_string())?;
        Ok(Self { stream, packets: Packets::default() })
    }

    /// Waits for the kernel's next upload reply.
    fn wait_for_reply(&mut self, timeout: Duration) -> Result<Option<(u32, UploadStatus)>, String> {
        let deadline = Instant::now() + timeout;
        let mut bytes = [0; 4096];
        while Instant::now() < deadline {
            let count = match self.stream.read(&mut bytes) {
                Ok(0) => return Err("the server closed the connection".to_string()),
                Ok(count) => count,
                Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => continue,
                Err(error) => return Err(error.to_string()),
            };
            for byte in &bytes[..count] {
                if let Some(Message::UploadReply { next_offset, status }) = self.packets.push(*byte) {
                    return Ok(Some((next_offset, status)));
                }
            }
        }
        Ok(None)
    }
}

fn post(body: &[u8]) -> Result<(), String> {
    let mut stream = TcpStream::connect(SEND_ADDRESS).map_err(|error| format!("can't reach cargo server: {error}"))?;
    let header = format!(
        "POST /send HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes()).and_then(|()| stream.write_all(body)).map_err(|error| error.to_string())?;
    let mut response = String::new();
    stream.read_to_string(&mut response).map_err(|error| error.to_string())?;
    if response.starts_with("HTTP/1.1 200") {
        Ok(())
    } else {
        Err(format!("the server refused: {}", response.lines().next().unwrap_or("no reply")))
    }
}

fn encode(message: Message) -> Vec<u8> {
    let mut packet = [0; MAX_PACKET_BYTES];
    let length = message.encode(&mut packet);
    packet[..length].to_vec()
}

fn chunk_at(contents: &[u8], offset: u32) -> Message {
    let start = offset as usize;
    let end = (start + CHUNK_BYTES).min(contents.len());
    let mut data = [0; CHUNK_BYTES];
    data[..end - start].copy_from_slice(&contents[start..end]);
    Message::UploadChunk { offset, length: (end - start) as u8, data }
}

/// Sends a message until something answers it.
fn exchange(viewer: &mut Viewer, packet: &[u8], timeout: Duration) -> Result<(u32, UploadStatus), String> {
    for _ in 0..MAX_RETRIES {
        post(packet)?;
        if let Some(reply) = viewer.wait_for_reply(timeout)? {
            return Ok(reply);
        }
    }
    Err("no reply from the kernel. Is it running one that takes uploads?".to_string())
}

fn upload(options: &Options) -> Result<(), String> {
    let contents = fs::read(&options.file).map_err(|error| format!("{}: {error}", options.file))?;
    let mut viewer = Viewer::connect()?;
    let started = Instant::now();
    let checksum = crc32(&contents) ^ u32::from(options.faults.wrong_crc);

    let size = u32::try_from(contents.len()).map_err(|_| "that's too big")?;
    if options.from_memory {
        let install = encode(Message::InstallFromMemory { target: options.target, size, crc32: checksum });
        let (_, status) = exchange(&mut viewer, &install, COMMIT_TIMEOUT)?;
        return report_finish(status, started);
    }
    let start = encode(Message::UploadStart { target: options.target, size, crc32: checksum });
    let (_, status) = exchange(&mut viewer, &start, REPLY_TIMEOUT)?;
    if status != UploadStatus::Receiving {
        return Err(format!("the kernel refused it: {status:?}"));
    }

    let mut offset = 0;
    let mut chunks_sent = 0;
    let mut resends = 0;
    while offset < size {
        let mut misses = 0;
        loop {
            chunks_sent += 1;
            let mut packet = encode(chunk_at(&contents, offset));
            let dropped = options.faults.drop_every.is_some_and(|every| chunks_sent % every == 0);
            if options.faults.corrupt_every.is_some_and(|every| chunks_sent % every == 0) {
                // A byte in the middle, as if damaged on the wire, keeping it free of zeros.
                let middle = packet.len() / 2;
                packet[middle] = packet[middle].wrapping_add(1).max(1);
            }
            if !dropped {
                post(&packet)?;
            }
            match viewer.wait_for_reply(REPLY_TIMEOUT)? {
                Some((_, UploadStatus::Refused)) => return Err("the kernel stopped the upload".to_string()),
                Some((next, _)) if next != offset => {
                    offset = next;
                    break;
                }
                _ => {
                    resends += 1;
                    misses += 1;
                    if misses == MAX_RETRIES {
                        return Err(format!("stuck at byte {offset}"));
                    }
                }
            }
        }
    }
    println!("upload: sent {size} bytes in {:.1} s, with {resends} resends", started.elapsed().as_secs_f32());

    let (_, status) = exchange(&mut viewer, &encode(Message::UploadCommit), COMMIT_TIMEOUT)?;
    report_finish(status, started)
}

fn report_finish(status: UploadStatus, started: Instant) -> Result<(), String> {
    match status {
        UploadStatus::Done => {
            println!("upload: on the card and read back correctly, {:.1} s in all", started.elapsed().as_secs_f32());
            Ok(())
        }
        UploadStatus::BadChecksum => Err("the checksum didn't match, so the card wasn't touched".to_string()),
        other => Err(format!("failed: {other:?}")),
    }
}

fn main() -> ExitCode {
    let command = match parse_options(env::args().skip(1)) {
        Ok(command) => command,
        Err(message) => {
            eprintln!("upload: {message}\n\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    let result = match command {
        Command::Upload(options) => upload(&options),
        Command::Reboot { trial } => post(&encode(Message::Reboot { trial })),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("upload: {message}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(arguments: &[&str]) -> Result<Options, String> {
        match parse_command(arguments)? {
            Command::Upload(options) => Ok(options),
            other => panic!("expected an upload, got {other:?}"),
        }
    }

    fn parse_command(arguments: &[&str]) -> Result<Command, String> {
        parse_options(arguments.iter().map(ToString::to_string))
    }

    #[test]
    fn a_trial_needs_no_long_flag_but_takes_no_faults() {
        assert_eq!(parse(&["trial"]).unwrap().target, Target::Trial);
        assert!(parse(&["trial", "--wrong-crc"]).is_err());
    }

    #[test]
    fn from_memory_can_check_the_crc_but_has_no_chunks_to_damage() {
        assert!(parse(&["scratch", "--from-memory", "--wrong-crc"]).unwrap().from_memory);
        assert!(parse(&["scratch", "--from-memory", "--drop-every", "3"]).is_err());
        assert!(parse(&["kernel", "--from-memory"]).is_err());
        assert!(parse(&["kernel", "--from-memory", "--really-replace-the-boot-kernel"]).unwrap().from_memory);
    }

    #[test]
    fn reboots_are_understood() {
        assert_eq!(parse_command(&["reboot"]), Ok(Command::Reboot { trial: false }));
        assert_eq!(parse_command(&["reboot", "--trial"]), Ok(Command::Reboot { trial: true }));
        assert!(parse_command(&["reboot", "--nope"]).is_err());
        assert!(parse_command(&["reboot", "--trial", "x"]).is_err());
    }

    #[test]
    fn the_boot_kernel_needs_the_long_flag() {
        assert!(parse(&["kernel"]).is_err());
        assert!(parse(&["kernel", "file"]).is_err());
        let options = parse(&["kernel", "--really-replace-the-boot-kernel"]).unwrap();
        assert_eq!(options.target, Target::Kernel);
    }

    #[test]
    fn test_faults_are_refused_on_the_boot_kernel() {
        assert!(parse(&["kernel", "--really-replace-the-boot-kernel", "--wrong-crc"]).is_err());
        assert!(parse(&["kernel", "--really-replace-the-boot-kernel", "--drop-every", "3"]).is_err());
    }

    #[test]
    fn scratch_takes_a_file_and_faults() {
        let options = parse(&["scratch", "a.bin", "--drop-every", "3", "--wrong-crc"]).unwrap();
        assert_eq!(options.file, "a.bin");
        assert_eq!(options.faults, Faults { corrupt_every: None, drop_every: Some(3), wrong_crc: true });
        assert_eq!(parse(&["scratch"]).unwrap().file, DEFAULT_FILE);
    }

    #[test]
    fn bad_arguments_are_refused() {
        for arguments in [&[][..], &["other"], &["scratch", "--nope"], &["scratch", "a", "b"], &["scratch", "--drop-every"], &["scratch", "--drop-every", "0"]] {
            assert!(parse(arguments).is_err(), "{arguments:?}");
        }
    }

    #[test]
    fn packets_are_picked_out_of_console_text() {
        let reply = Message::UploadReply { next_offset: 480, status: UploadStatus::Receiving };
        let mut bytes = b"Upload: some text\r\n".to_vec();
        bytes.extend(encode(reply));
        bytes.extend(b"more text");
        bytes.extend(encode(reply));
        let mut packets = Packets::default();
        let found: Vec<Message> = bytes.iter().filter_map(|byte| packets.push(*byte)).collect();
        assert_eq!(found, [reply, reply]);
    }

    #[test]
    fn chunks_cover_the_file_exactly() {
        let contents = vec![9; CHUNK_BYTES + 5];
        let Message::UploadChunk { length, .. } = chunk_at(&contents, 0) else { panic!() };
        assert_eq!(usize::from(length), CHUNK_BYTES);
        let Message::UploadChunk { offset, length, .. } = chunk_at(&contents, CHUNK_BYTES as u32) else { panic!() };
        assert_eq!((offset as usize, length), (CHUNK_BYTES, 5));
    }
}
