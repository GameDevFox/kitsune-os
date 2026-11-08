//! Shares this PC's line-in, where the Pi's headphone jack is wired. `parec` records it
//! as raw 48 kHz stereo 16-bit samples, which go to each listener in 20 ms chunks.
//! Unlike video, every chunk matters, so listeners get them in order rather than only
//! the newest, unless they fall far enough behind that the oldest are gone.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

/// What `sound.js` on the page expects. Changing these means changing it too.
const SAMPLE_RATE: &str = "48000";
const CHANNELS: &str = "2";
const FORMAT: &str = "s16le";
/// 20 ms of 48 kHz stereo 16-bit samples: 960 frames of 4 bytes.
const CHUNK_BYTES: usize = 3840;
/// What `parec` may hold before handing samples over, as more is more delay.
const RECORD_LATENCY_MS: &str = "20";
/// How many chunks are kept for a listener who's briefly slow: 640 ms.
const KEPT_CHUNKS: usize = 32;
/// A stream with no new chunk for this long ends, as the recording has stopped.
const STREAM_IDLE_LIMIT: Duration = Duration::from_secs(10);
const RESTART_DELAY: Duration = Duration::from_secs(2);

/// The most recent chunks, numbered from 1, shared between the recording and listeners.
#[derive(Default)]
pub struct RecentAudio {
    chunks: Mutex<Kept>,
    changed: Condvar,
}

#[derive(Default)]
struct Kept {
    /// The number of the newest chunk, or 0 before the first.
    newest: u64,
    chunks: VecDeque<Arc<Vec<u8>>>,
}

impl Kept {
    /// The chunk after `seen`, or the oldest kept if `seen` has already gone.
    fn after(&self, seen: u64) -> Option<(u64, Arc<Vec<u8>>)> {
        let oldest = self.newest + 1 - self.chunks.len() as u64;
        let wanted = (seen + 1).max(oldest);
        let index = usize::try_from(wanted - oldest).ok()?;
        self.chunks.get(index).map(|chunk| (wanted, Arc::clone(chunk)))
    }
}

impl RecentAudio {
    pub fn publish(&self, chunk: Vec<u8>) {
        let mut kept = self.chunks.lock().unwrap();
        kept.newest += 1;
        kept.chunks.push_back(Arc::new(chunk));
        if kept.chunks.len() > KEPT_CHUNKS {
            kept.chunks.pop_front();
        }
        self.changed.notify_all();
    }

    /// The number of the newest chunk, so that a new listener starts from now.
    fn newest(&self) -> u64 {
        self.chunks.lock().unwrap().newest
    }

    /// Waits for the chunk after `seen`, skipping ahead to the oldest kept one if it
    /// has already gone. Returns `None` if none comes within `timeout`.
    fn wait_after(&self, seen: u64, timeout: Duration) -> Option<(u64, Arc<Vec<u8>>)> {
        let guard = self.chunks.lock().unwrap();
        let (guard, _) = self.changed.wait_timeout_while(guard, timeout, |kept| kept.newest <= seen).unwrap();
        guard.after(seen)
    }
}

/// What goes ahead of the samples, as they're written straight to the socket.
pub fn stream_head() -> String {
    "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n".to_string()
}

/// Writes each chunk from now on, in order, flushing each, until the listener goes away
/// or the recording stops.
pub fn write_stream(audio: &RecentAudio, output: &mut impl Write) -> std::io::Result<()> {
    write_stream_until_idle(audio, output, STREAM_IDLE_LIMIT)
}

fn write_stream_until_idle(audio: &RecentAudio, output: &mut impl Write, idle_limit: Duration) -> std::io::Result<()> {
    let mut seen = audio.newest();
    while let Some((number, chunk)) = audio.wait_after(seen, idle_limit) {
        seen = number;
        output.write_all(&chunk)?;
        output.flush()?;
    }
    Ok(())
}

/// Runs forever, restarting `parec` whenever it stops. Only logs a repeating failure
/// once, as with video.
pub fn run_capture(source: &str, audio: &RecentAudio) {
    let mut announced_failure = false;
    loop {
        let child = Command::new("parec")
            .args([&format!("--device={source}"), "--raw", &format!("--format={FORMAT}")])
            .args([&format!("--rate={SAMPLE_RATE}"), &format!("--channels={CHANNELS}")])
            .arg(format!("--latency-msec={RECORD_LATENCY_MS}"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();

        match child {
            Ok(mut child) => {
                let mut got_audio = false;
                if let Some(mut stdout) = child.stdout.take() {
                    let mut chunk = vec![0; CHUNK_BYTES];
                    while stdout.read_exact(&mut chunk).is_ok() {
                        if !got_audio {
                            println!("server: receiving audio from {source}");
                            got_audio = true;
                            announced_failure = false;
                        }
                        audio.publish(chunk.clone());
                    }
                }
                let _ = child.kill();
                let _ = child.wait();
                if got_audio || !announced_failure {
                    println!("server: no audio from {source}; retrying");
                    announced_failure = true;
                }
            }
            Err(error) => {
                if !announced_failure {
                    println!("server: could not run parec: {error}");
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

    const IDLE: Duration = Duration::from_millis(10);
    /// A stream that never ended would otherwise fill memory; see `video.rs`.
    const MOST_FLUSHES: usize = 64;

    #[derive(Default)]
    struct Socket {
        pending: Vec<u8>,
        flushed: Vec<Vec<u8>>,
    }

    impl Write for Socket {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.pending.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            assert!(self.flushed.len() < MOST_FLUSHES, "the stream never ended");
            self.flushed.push(std::mem::take(&mut self.pending));
            Ok(())
        }
    }

    fn chunk(value: u8) -> Vec<u8> {
        vec![value; 4]
    }

    /// Streams while `publish` runs on another thread, once the stream has started.
    fn stream_while(audio: Arc<RecentAudio>, publish: impl FnOnce(&RecentAudio) + Send + 'static) -> Vec<Vec<u8>> {
        let publisher = Arc::clone(&audio);
        let later = std::thread::spawn(move || {
            std::thread::sleep(IDLE / 2);
            publish(&publisher);
        });
        let mut socket = Socket::default();
        write_stream_until_idle(&audio, &mut socket, IDLE * 5).unwrap();
        later.join().unwrap();
        socket.flushed
    }

    #[test]
    fn every_chunk_arrives_in_order_and_whole() {
        let audio = Arc::new(RecentAudio::default());
        let got = stream_while(audio, |audio| (1..=5).for_each(|value| audio.publish(chunk(value))));
        assert_eq!(got, (1..=5).map(chunk).collect::<Vec<_>>());
    }

    #[test]
    fn a_new_listener_starts_from_now() {
        let audio = Arc::new(RecentAudio::default());
        audio.publish(chunk(1));
        audio.publish(chunk(2));
        let got = stream_while(audio, |audio| audio.publish(chunk(3)));
        assert_eq!(got, vec![chunk(3)]);
    }

    #[test]
    fn a_listener_too_far_behind_skips_to_the_oldest_kept() {
        let audio = RecentAudio::default();
        let total = KEPT_CHUNKS as u64 + 3;
        for value in 1..=total {
            audio.publish(chunk(value as u8));
        }
        let (number, first) = audio.wait_after(0, IDLE).unwrap();
        assert_eq!(number, total - KEPT_CHUNKS as u64 + 1);
        assert_eq!(*first, chunk(number as u8));
    }

    #[test]
    fn a_listener_keeping_up_gets_the_next_chunk() {
        let audio = RecentAudio::default();
        (1..=3).for_each(|value| audio.publish(chunk(value)));
        let (number, next) = audio.wait_after(1, IDLE).unwrap();
        assert_eq!((number, (*next).clone()), (2, chunk(2)));
    }

    #[test]
    fn no_chunks_means_the_stream_ends() {
        let audio = RecentAudio::default();
        assert!(audio.wait_after(0, IDLE).is_none());
        let mut socket = Socket::default();
        write_stream_until_idle(&audio, &mut socket, IDLE).unwrap();
        assert!(socket.flushed.is_empty());
    }

    #[test]
    fn only_so_many_chunks_are_kept() {
        let audio = RecentAudio::default();
        for value in 0..(KEPT_CHUNKS * 2) {
            audio.publish(chunk(value as u8));
        }
        assert_eq!(audio.chunks.lock().unwrap().chunks.len(), KEPT_CHUNKS);
    }
}
