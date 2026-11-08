//! Shares the HDMI capture card. `ffmpeg` reads it, as the card already produces
//! JPEG frames, and viewers get whichever frame is newest, so a slow viewer skips
//! frames rather than holding anyone else up.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

const JPEG_START: [u8; 2] = [0xFF, 0xD8];
const JPEG_END: [u8; 2] = [0xFF, 0xD9];
/// Far above any real frame, so that garbage can't grow the buffer forever.
const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

const READ_BUFFER_SIZE: usize = 64 * 1024;
const VIDEO_SIZE: &str = "1920x1080";
const FRAME_RATE: &str = "30";
const RESTART_DELAY: Duration = Duration::from_secs(2);

const STREAM_BOUNDARY: &str = "frame";
/// A stream with no new frame for this long ends, rather than hanging silently.
const STREAM_IDLE_LIMIT: Duration = Duration::from_secs(10);

/// Pulls whole JPEG images out of a byte stream.
#[derive(Default)]
pub struct FrameSplitter {
    buffer: Vec<u8>,
    /// How much of `buffer` is known not to hold a frame's end, so that it's only searched once.
    scanned: usize,
}

fn find(haystack: &[u8], needle: &[u8; 2], from: usize) -> Option<usize> {
    let tail = haystack.get(from..)?;
    tail.windows(needle.len()).position(|window| window == needle).map(|index| index + from)
}

impl FrameSplitter {
    pub fn push(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
        self.buffer.extend_from_slice(bytes);

        let mut frames = Vec::new();
        while let Some(start) = find(&self.buffer, &JPEG_START, 0) {
            let from = self.scanned.max(start + JPEG_START.len());
            let Some(end) = find(&self.buffer, &JPEG_END, from) else {
                self.buffer.drain(..start);
                if self.buffer.len() > MAX_FRAME_BYTES {
                    self.buffer.clear();
                }
                // The last byte could be the first half of the end marker.
                self.scanned = self.buffer.len().saturating_sub(JPEG_END.len() - 1);
                return frames;
            };
            let end = end + JPEG_END.len();
            frames.push(self.buffer[start..end].to_vec());
            self.buffer.drain(..end);
            self.scanned = 0;
        }

        // No start marker left. Only a final 0xFF could still become one.
        let keep_last = self.buffer.last() == Some(&JPEG_START[0]);
        let keep_from = self.buffer.len() - usize::from(keep_last);
        self.buffer.drain(..keep_from);
        self.scanned = 0;
        frames
    }
}

struct Frame {
    number: u64,
    data: Arc<Vec<u8>>,
    captured: Instant,
}

/// The newest frame, shared between the capture and all viewers.
#[derive(Default)]
pub struct LatestFrame {
    frame: Mutex<Option<Frame>>,
    changed: Condvar,
}

impl LatestFrame {
    pub fn publish(&self, data: Vec<u8>) {
        let mut frame = self.frame.lock().unwrap();
        let number = frame.as_ref().map_or(0, |frame| frame.number) + 1;
        *frame = Some(Frame { number, data: Arc::new(data), captured: Instant::now() });
        self.changed.notify_all();
    }

    /// Returns the newest frame, unless it's older than `max_age`. A frozen image
    /// must not pass for what's on the screen now.
    pub fn current(&self, max_age: Duration) -> Option<Arc<Vec<u8>>> {
        let frame = self.frame.lock().unwrap();
        frame.as_ref().filter(|frame| frame.captured.elapsed() <= max_age).map(|frame| Arc::clone(&frame.data))
    }

    /// Waits for a frame numbered above `seen`, and not older than `max_age`.
    fn wait_newer(&self, seen: u64, max_age: Duration, timeout: Duration) -> Option<(u64, Arc<Vec<u8>>)> {
        let guard = self.frame.lock().unwrap();
        let is_old = |frame: &mut Option<Frame>| frame.as_ref().is_none_or(|frame| frame.number <= seen || frame.captured.elapsed() > max_age);
        let (guard, _) = self.changed.wait_timeout_while(guard, timeout, is_old).unwrap();
        guard.as_ref().filter(|frame| frame.number > seen && frame.captured.elapsed() <= max_age).map(|frame| (frame.number, Arc::clone(&frame.data)))
    }
}

/// What goes ahead of the frames, as they're written straight to the socket.
pub fn stream_head() -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: multipart/x-mixed-replace; boundary={STREAM_BOUNDARY}\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n\r\n"
    )
}

/// Writes each new frame once, as a `multipart/x-mixed-replace` part, until the viewer goes
/// away or the frames stop. Each is flushed whole, as a frame's end left in a buffer would
/// only arrive with the next frame, a whole frame late. A viewer too slow to keep up finds
/// that its next frame is the newest one, skipping those between. A frame older than
/// `max_age` isn't sent, as a frozen image must not pass for what's on the screen now.
pub fn write_stream(frames: &LatestFrame, output: &mut impl Write, max_age: Duration) -> std::io::Result<()> {
    write_stream_until_idle(frames, output, max_age, STREAM_IDLE_LIMIT)
}

fn write_stream_until_idle(frames: &LatestFrame, output: &mut impl Write, max_age: Duration, idle_limit: Duration) -> std::io::Result<()> {
    let mut seen = 0;
    while let Some((number, data)) = frames.wait_newer(seen, max_age, idle_limit) {
        seen = number;
        write!(output, "--{STREAM_BOUNDARY}\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n", data.len())?;
        output.write_all(&data)?;
        output.write_all(b"\r\n")?;
        output.flush()?;
    }
    Ok(())
}

/// Runs forever, restarting `ffmpeg` whenever it stops. Only logs a repeating
/// failure once, since retrying while the card is unplugged is expected.
pub fn run_capture(device: &str, frames: &LatestFrame) {
    let mut announced_failure = false;
    loop {
        let child = Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-f", "v4l2", "-input_format", "mjpeg"])
            .args(["-video_size", VIDEO_SIZE, "-framerate", FRAME_RATE, "-i", device])
            .args(["-c", "copy", "-f", "mjpeg", "-"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();

        match child {
            Ok(mut child) => {
                let mut got_frames = false;
                if let Some(mut stdout) = child.stdout.take() {
                    let mut splitter = FrameSplitter::default();
                    let mut buffer = vec![0; READ_BUFFER_SIZE];
                    while let Ok(count) = stdout.read(&mut buffer) {
                        if count == 0 {
                            break;
                        }
                        for frame in splitter.push(&buffer[..count]) {
                            if !got_frames {
                                println!("server: receiving video from {device}");
                                got_frames = true;
                                announced_failure = false;
                            }
                            frames.publish(frame);
                        }
                    }
                }
                let _ = child.wait();
                if got_frames || !announced_failure {
                    println!("server: no video from {device}; retrying");
                    announced_failure = true;
                }
            }
            Err(error) => {
                if !announced_failure {
                    println!("server: could not run ffmpeg: {error}");
                    announced_failure = true;
                }
            }
        }
        std::thread::sleep(RESTART_DELAY);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &[u8] = &[0xFF, 0xD8, 1, 2, 0xFF, 0xD9];
    const B: &[u8] = &[0xFF, 0xD8, 3, 0xFF, 0xD9];

    const IDLE: Duration = Duration::from_millis(10);
    const MAX_AGE: Duration = Duration::from_secs(60);

    /// A stream that never stopped would otherwise fill memory until the machine locked up,
    /// which a mutation did on 2026-11-06. No test here needs more than this.
    const MOST_FLUSHES: usize = 16;

    /// Records what has been flushed, as that's all a viewer would receive.
    #[derive(Default)]
    struct Socket {
        buffered: Vec<u8>,
        flushed: Vec<Vec<u8>>,
    }

    impl Write for Socket {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.buffered.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            if self.flushed.len() == MOST_FLUSHES {
                return Err(std::io::ErrorKind::OutOfMemory.into());
            }
            self.flushed.push(std::mem::take(&mut self.buffered));
            Ok(())
        }
    }

    fn part(frame: &[u8]) -> Vec<u8> {
        let header = format!("--frame\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n", frame.len());
        [header.as_bytes(), frame, b"\r\n"].concat()
    }

    #[test]
    fn a_whole_frame_comes_out_unchanged() {
        assert_eq!(FrameSplitter::default().push(A), [A]);
    }

    #[test]
    fn two_frames_in_one_read_both_come_out() {
        assert_eq!(FrameSplitter::default().push(&[A, B].concat()), [A, B]);
    }

    #[test]
    fn a_frame_split_at_every_position_still_comes_out() {
        for split in 0..=A.len() {
            let mut splitter = FrameSplitter::default();
            let mut frames = splitter.push(&A[..split]);
            frames.extend(splitter.push(&A[split..]));
            assert_eq!(frames, [A], "split at {split}");
        }
    }

    #[test]
    fn junk_before_a_frame_is_dropped() {
        assert_eq!(FrameSplitter::default().push(&[&[9, 9, 0xFF, 7], A].concat()), [A]);
    }

    #[test]
    fn a_runaway_frame_cannot_grow_forever() {
        let mut splitter = FrameSplitter::default();
        splitter.push(&JPEG_START);
        splitter.push(&vec![0; MAX_FRAME_BYTES + 1]);
        assert!(splitter.buffer.is_empty());
    }

    #[test]
    fn no_frame_yet_means_nothing_current() {
        assert!(LatestFrame::default().current(Duration::from_secs(1)).is_none());
    }

    #[test]
    fn the_newest_frame_is_current() {
        let frames = LatestFrame::default();
        frames.publish(A.to_vec());
        frames.publish(B.to_vec());
        assert_eq!(frames.current(Duration::from_secs(60)).unwrap().as_slice(), B);
    }

    #[test]
    fn a_frame_older_than_the_limit_is_not_current() {
        let frames = LatestFrame::default();
        frames.publish(A.to_vec());
        std::thread::sleep(Duration::from_millis(20));
        assert!(frames.current(Duration::from_millis(1)).is_none());
    }

    #[test]
    fn a_stream_flushes_each_frame_whole_and_sends_it_once() {
        let frames = Arc::new(LatestFrame::default());
        frames.publish(A.to_vec());
        let publisher = Arc::clone(&frames);
        let later = std::thread::spawn(move || {
            std::thread::sleep(IDLE / 2);
            publisher.publish(B.to_vec());
        });
        let mut socket = Socket::default();
        write_stream_until_idle(&frames, &mut socket, MAX_AGE, IDLE).unwrap();
        later.join().unwrap();
        assert_eq!(socket.flushed, [part(A), part(B)]);
        assert!(socket.buffered.is_empty());
    }

    #[test]
    fn a_stream_waits_past_an_old_frame_for_a_new_one() {
        let frames = Arc::new(LatestFrame::default());
        frames.publish(A.to_vec());
        std::thread::sleep(IDLE);
        let publisher = Arc::clone(&frames);
        let later = std::thread::spawn(move || {
            std::thread::sleep(IDLE);
            publisher.publish(B.to_vec());
        });
        let mut socket = Socket::default();
        write_stream_until_idle(&frames, &mut socket, IDLE / 2, IDLE * 5).unwrap();
        later.join().unwrap();
        assert_eq!(socket.flushed, [part(B)]);
    }

    #[test]
    fn a_short_frame_right_after_a_long_one_is_found() {
        let mut long = vec![0xFF, 0xD8];
        long.extend([7; 100]);
        long.extend([0xFF, 0xD9]);
        let mut splitter = FrameSplitter::default();
        assert!(splitter.push(&long[..long.len() - 1]).is_empty());
        let frames = splitter.push(&[&long[long.len() - 1..], B].concat());
        assert_eq!(frames, [long.as_slice(), B]);
    }

    #[test]
    fn a_stream_with_no_frames_ends() {
        let mut socket = Socket::default();
        write_stream_until_idle(&LatestFrame::default(), &mut socket, MAX_AGE, IDLE).unwrap();
        assert!(socket.flushed.is_empty());
    }

    #[test]
    fn a_viewer_going_away_ends_the_stream() {
        struct Gone;
        impl Write for Gone {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let frames = LatestFrame::default();
        frames.publish(A.to_vec());
        assert!(write_stream_until_idle(&frames, &mut Gone, MAX_AGE, MAX_AGE).is_err());
    }

    #[test]
    fn a_stream_never_starts_with_an_old_frame() {
        let frames = LatestFrame::default();
        frames.publish(A.to_vec());
        std::thread::sleep(IDLE);
        let mut socket = Socket::default();
        write_stream_until_idle(&frames, &mut socket, IDLE / 2, IDLE).unwrap();
        assert!(socket.flushed.is_empty());
    }

    #[test]
    fn an_end_marker_split_between_reads_is_still_found() {
        let mut splitter = FrameSplitter::default();
        assert!(splitter.push(&A[..A.len() - 1]).is_empty());
        assert_eq!(splitter.push(&A[A.len() - 1..]), [A]);
    }
}
