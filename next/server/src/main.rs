//! Owns the serial port, the HDMI capture card and the line-in. Viewers receive serial
//! output over TCP, `POST /send` writes to the port, and video and sound are over HTTP.
//! Linux only, as it relies on `stty -F` and the Linux value of `O_NOCTTY`.

mod audio;
mod hub;
mod input;
mod send;
mod video;

use std::fs::{File, OpenOptions};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::process::{Command, ExitCode};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use audio::RecentAudio;
use hub::Hub;
use video::LatestFrame;

const DEFAULT_DEVICE: &str = "/dev/ttyACM0";
const DEFAULT_VIDEO_DEVICE: &str = "/dev/video0";
/// This PC's line-in, where the Pi's headphone jack is wired.
const DEFAULT_AUDIO_SOURCE: &str = "alsa_input.usb-Generic_USB_Audio-00.HiFi__Line__source";
const DEFAULT_BAUD: u32 = 115200;
const DEFAULT_VIEWER_PORT: u16 = 8765;
const DEFAULT_HTTP_PORT: u16 = 8766;
/// All interfaces, because the user is often on another machine.
const LISTEN_ADDRESS: &str = "0.0.0.0";

const READ_BUFFER_SIZE: usize = 4096;
/// How much output a viewer sees when it joins.
const HISTORY_SIZE: usize = 64 * 1024;
const REOPEN_DELAY: Duration = Duration::from_secs(1);
/// A viewer that can't take data for this long is dropped.
const VIEWER_WRITE_TIMEOUT: Duration = Duration::from_secs(1);

/// Stops the port becoming this process's controlling terminal.
const O_NOCTTY: i32 = 0o400;

/// How long a read waits for data, in tenths of a second, so that an idle read
/// still returns and can notice the device disappearing.
const READ_TIMEOUT_TENTHS: &str = "1";

/// `/frame.jpg` refuses anything older, so a stopped capture can't pass for the
/// screen as it is now.
const FRAME_MAX_AGE: Duration = Duration::from_secs(2);

const STATUS_UNAVAILABLE: u16 = 503;

/// The page a browser gets: video with zoom, pan and a pixel readout.
const VIDEO_PAGE: &str = include_str!("video.html");
/// The page's sound player, which it loads from `/sound.js`.
const SOUND_SCRIPT: &str = include_str!("sound.js");

/// Identifies a page's contents, so an open tab can tell when the server has a newer one.
fn page_version(page: &str) -> String {
    let mut hasher = DefaultHasher::new();
    page.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

const USAGE: &str = "\
Owns a serial port and an HDMI capture card, and shares them. Viewers such as
`cargo uart` receive serial output, `POST /send` writes to the port, and a browser
can open the HTTP port to watch the video and hear the line-in. `GET /frame.jpg`
returns the latest frame, and `GET /audio` streams raw 48 kHz stereo 16-bit samples.

Usage: cargo server [OPTIONS]

Options:
  --device <PATH>     Serial device [default: /dev/ttyACM0]
  --baud <RATE>       Baud rate [default: 115200]
  --port <PORT>       TCP port for viewers [default: 8765]
  --http-port <PORT>  HTTP port [default: 8766]
  --video <PATH>      Capture card [default: /dev/video0]
  --audio <SOURCE>    PulseAudio or PipeWire source to share [default: this PC's line-in]
  --help              Show this message";

type SharedHub = Arc<Mutex<Hub<TcpStream>>>;
/// Empty while the serial device is unavailable.
type SharedWriter = Arc<Mutex<Option<File>>>;

#[derive(Debug, PartialEq)]
struct Options {
    device: String,
    baud: u32,
    viewer_port: u16,
    http_port: u16,
    video_device: String,
    audio_source: String,
}

/// Returns `None` if the user asked for help.
fn parse_options(args: impl IntoIterator<Item = String>) -> Result<Option<Options>, String> {
    let mut options = Options {
        device: DEFAULT_DEVICE.to_string(),
        baud: DEFAULT_BAUD,
        viewer_port: DEFAULT_VIEWER_PORT,
        http_port: DEFAULT_HTTP_PORT,
        video_device: DEFAULT_VIDEO_DEVICE.to_string(),
        audio_source: DEFAULT_AUDIO_SOURCE.to_string(),
    };
    let mut args = args.into_iter();

    while let Some(flag) = args.next() {
        if flag == "--help" {
            return Ok(None);
        }
        let value = args.next().ok_or(format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--device" => options.device = value,
            "--baud" => options.baud = parse_number(&flag, &value)?,
            "--port" => options.viewer_port = parse_number(&flag, &value)?,
            "--http-port" => options.http_port = parse_number(&flag, &value)?,
            "--video" => options.video_device = value,
            "--audio" => options.audio_source = value,
            _ => return Err(format!("unknown option: {flag}")),
        }
    }
    Ok(Some(options))
}

fn parse_number<T: std::str::FromStr>(flag: &str, value: &str) -> Result<T, String> {
    value.parse().map_err(|_| format!("{flag} needs a number, got '{value}'"))
}

/// Returns separate handles for reading and writing the port.
fn open_port(options: &Options) -> Result<(File, File), String> {
    let device = &options.device;
    let baud = options.baud.to_string();
    let stty = Command::new("stty")
        .args(["-F", device, &baud, "raw", "-echo", "clocal", "min", "0", "time", READ_TIMEOUT_TENTHS])
        .output()
        .map_err(|error| format!("could not run stty: {error}"))?;
    if !stty.status.success() {
        return Err(String::from_utf8_lossy(&stty.stderr).trim().to_string());
    }

    let reader = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(O_NOCTTY)
        .open(device)
        .map_err(|error| format!("could not open {device}: {error}"))?;
    let writer = reader.try_clone().map_err(|error| format!("could not share {device}: {error}"))?;
    Ok((reader, writer))
}

/// Shares bytes from the port until it fails, and returns the reason.
fn share_port(device: &str, port: &mut File, hub: &SharedHub) -> String {
    let mut buffer = [0; READ_BUFFER_SIZE];
    loop {
        match port.read(&mut buffer) {
            // Zero bytes means the read timed out, or that the device was unplugged.
            Ok(0) => {
                if !Path::new(device).exists() {
                    return "device disappeared".to_string();
                }
            }
            Ok(count) => hub.lock().unwrap().broadcast(&buffer[..count]),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return error.to_string(),
        }
    }
}

/// Runs forever, reopening the port whenever it is lost. Only logs a repeating
/// failure once, since retrying while the adapter is unplugged is expected.
fn run_port(options: &Options, hub: &SharedHub, writer: &SharedWriter) -> ! {
    let mut last_error = None;
    loop {
        match open_port(options) {
            Ok((mut reader, port_writer)) => {
                println!("server: reading {} at {} baud", options.device, options.baud);
                last_error = None;
                *writer.lock().unwrap() = Some(port_writer);
                let reason = share_port(&options.device, &mut reader, hub);
                *writer.lock().unwrap() = None;
                println!("server: lost {}: {reason}", options.device);
            }
            Err(error) => {
                if last_error.as_ref() != Some(&error) {
                    println!("server: waiting for {}: {error}", options.device);
                    last_error = Some(error);
                }
            }
        }
        std::thread::sleep(REOPEN_DELAY);
    }
}

fn accept_viewers(listener: TcpListener, hub: SharedHub) {
    for connection in listener.incoming() {
        let Ok(viewer) = connection else { continue };
        let name = viewer.peer_addr().map_or("unknown".to_string(), |address| address.to_string());
        // These only tune the connection, so carry on if they fail.
        let _ = viewer.set_nodelay(true);
        let _ = viewer.set_write_timeout(Some(VIEWER_WRITE_TIMEOUT));

        let mut hub = hub.lock().unwrap();
        match hub.add_viewer(viewer) {
            Ok(()) => println!("server: viewer {name} connected ({} active)", hub.viewer_count()),
            Err(error) => println!("server: viewer {name} failed to connect: {error}"),
        }
    }
}

fn header(field: &str, value: &str) -> tiny_http::Header {
    tiny_http::Header::from_bytes(field, value).expect("these headers are valid")
}

/// Runs on its own thread, as a stream lasts as long as the viewer keeps watching.
fn serve_video_stream(request: tiny_http::Request, frames: Arc<LatestFrame>) {
    let mut socket = request.into_writer();
    // A viewer closing its browser ends the stream with an error, which is normal.
    let _ = socket.write_all(video::stream_head().as_bytes()).and_then(|()| video::write_stream(&frames, &mut socket, FRAME_MAX_AGE));
}

/// Runs on its own thread, as a stream lasts as long as the listener keeps listening.
fn serve_audio_stream(request: tiny_http::Request, audio: Arc<RecentAudio>) {
    let mut socket = request.into_writer();
    // A listener going away ends the stream with an error, which is normal.
    let _ = socket.write_all(audio::stream_head().as_bytes()).and_then(|()| audio::write_stream(&audio, &mut socket));
}

fn serve_frame(request: tiny_http::Request, frames: &LatestFrame) -> std::io::Result<()> {
    match frames.current(FRAME_MAX_AGE) {
        Some(frame) => request.respond(
            tiny_http::Response::from_data(frame.as_slice())
                .with_header(header("Content-Type", "image/jpeg"))
                .with_header(header("Cache-Control", "no-store")),
        ),
        None => request.respond(
            tiny_http::Response::from_string("no current video frame\n").with_status_code(STATUS_UNAVAILABLE),
        ),
    }
}

fn serve_http(server: tiny_http::Server, writer: SharedWriter, frames: Arc<LatestFrame>, audio: Arc<RecentAudio>) {
    for mut request in server.incoming_requests() {
        let method = request.method().to_string();
        let path = request.url().to_string();

        if method == "GET" {
            match path.as_str() {
                "/" => {
                    let page = tiny_http::Response::from_string(VIDEO_PAGE)
                        .with_header(header("Content-Type", "text/html; charset=utf-8"));
                    let _ = request.respond(page);
                    continue;
                }
                "/version" => {
                    let version = tiny_http::Response::from_string(page_version(&format!("{VIDEO_PAGE}{SOUND_SCRIPT}")))
                        .with_header(header("Cache-Control", "no-store"));
                    let _ = request.respond(version);
                    continue;
                }
                "/video" => {
                    let frames = Arc::clone(&frames);
                    std::thread::spawn(move || serve_video_stream(request, frames));
                    continue;
                }
                "/sound.js" => {
                    let script = tiny_http::Response::from_string(SOUND_SCRIPT)
                        .with_header(header("Content-Type", "text/javascript; charset=utf-8"))
                        .with_header(header("Cache-Control", "no-store"));
                    let _ = request.respond(script);
                    continue;
                }
                "/audio" => {
                    let audio = Arc::clone(&audio);
                    std::thread::spawn(move || serve_audio_stream(request, audio));
                    continue;
                }
                "/frame.jpg" => {
                    let _ = serve_frame(request, &frames);
                    continue;
                }
                _ => {}
            }
        }

        if let Some(status) = input::handle(&method, &path, request.as_reader(), &writer) {
            let _ = request.respond(tiny_http::Response::empty(status));
            continue;
        }

        let client = request.remote_addr().map_or("unknown".to_string(), |address| address.to_string());
        let content_type = request
            .headers()
            .iter()
            .find(|header| header.field.equiv("Content-Type"))
            .map(|header| header.value.as_str().to_string());

        let reply = send::handle(&method, &path, content_type.as_deref(), request.as_reader(), &writer);
        println!("server: {client} {method} {path} -> {} {}", reply.status, reply.body.trim_end());

        let response = tiny_http::Response::from_string(reply.body).with_status_code(reply.status);
        if let Err(error) = request.respond(response) {
            println!("server: could not reply to {client}: {error}");
        }
    }
}

fn main() -> ExitCode {
    let options = match parse_options(std::env::args().skip(1)) {
        Ok(Some(options)) => options,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("server: {message}\n\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };

    let listener = match TcpListener::bind((LISTEN_ADDRESS, options.viewer_port)) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("server: could not listen on port {}: {error}", options.viewer_port);
            return ExitCode::FAILURE;
        }
    };
    let http_server = match tiny_http::Server::http((LISTEN_ADDRESS, options.http_port)) {
        Ok(server) => server,
        Err(error) => {
            eprintln!("server: could not listen on port {}: {error}", options.http_port);
            return ExitCode::FAILURE;
        }
    };
    println!("server: viewers can connect on port {}", options.viewer_port);
    println!("server: POST /send and video on port {}", options.http_port);

    let hub: SharedHub = Arc::new(Mutex::new(Hub::new(HISTORY_SIZE)));
    let writer: SharedWriter = Arc::new(Mutex::new(None));

    let viewer_hub = Arc::clone(&hub);
    std::thread::spawn(move || accept_viewers(listener, viewer_hub));
    let frames = Arc::new(LatestFrame::default());
    let capture_frames = Arc::clone(&frames);
    let video_device = options.video_device.clone();
    std::thread::spawn(move || video::run_capture(&video_device, &capture_frames));

    let audio = Arc::new(RecentAudio::default());
    let capture_audio = Arc::clone(&audio);
    let audio_source = options.audio_source.clone();
    std::thread::spawn(move || audio::run_capture(&audio_source, &capture_audio));

    let http_writer = Arc::clone(&writer);
    std::thread::spawn(move || serve_http(http_server, http_writer, frames, audio));

    run_port(&options, &hub, &writer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_version_changes_only_with_the_page() {
        assert_eq!(page_version("one"), page_version("one"));
        assert_ne!(page_version("one"), page_version("two"));
    }

    fn parse(args: &[&str]) -> Result<Option<Options>, String> {
        parse_options(args.iter().map(|arg| arg.to_string()))
    }

    #[test]
    fn no_options_gives_the_defaults() {
        // Literal values: comparing against the constants would pass whatever they held.
        let expected = Options {
            device: "/dev/ttyACM0".to_string(),
            baud: 115200,
            viewer_port: 8765,
            http_port: 8766,
            video_device: "/dev/video0".to_string(),
            audio_source: "alsa_input.usb-Generic_USB_Audio-00.HiFi__Line__source".to_string(),
        };
        assert_eq!(parse(&[]), Ok(Some(expected)));
    }

    #[test]
    fn options_override_the_defaults() {
        let expected = Options {
            device: "/dev/x".to_string(),
            baud: 9600,
            viewer_port: 1234,
            http_port: 1235,
            video_device: "/dev/v".to_string(),
            audio_source: "a".to_string(),
        };
        let args = ["--device", "/dev/x", "--baud", "9600", "--port", "1234", "--http-port", "1235", "--video", "/dev/v", "--audio", "a"];
        assert_eq!(parse(&args), Ok(Some(expected)));
    }

    #[test]
    fn bad_input_is_rejected() {
        assert!(parse(&["--nope", "x"]).is_err());
        assert!(parse(&["--baud"]).is_err());
        assert!(parse(&["--port", "99999"]).is_err());
    }
}
