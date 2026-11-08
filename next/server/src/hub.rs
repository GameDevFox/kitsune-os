//! Sends bytes from the serial port to every connected viewer.

use std::collections::VecDeque;
use std::io::{self, Write};

/// Each viewer first receives this, the history's length, and a newline, so it
/// can tell old output from new. It's readable text, so `nc` works as a viewer.
pub const HEADER_PREFIX: &str = "kitsune-uart history=";

pub struct Hub<W> {
    history: VecDeque<u8>,
    history_capacity: usize,
    viewers: Vec<W>,
}

impl<W: Write> Hub<W> {
    pub fn new(history_capacity: usize) -> Self {
        Self { history: VecDeque::new(), history_capacity, viewers: Vec::new() }
    }

    /// Sends the header and history, then adds the viewer for future bytes.
    pub fn add_viewer(&mut self, mut viewer: W) -> io::Result<()> {
        writeln!(viewer, "{HEADER_PREFIX}{}", self.history.len())?;
        viewer.write_all(self.history.make_contiguous())?;
        self.viewers.push(viewer);
        Ok(())
    }

    /// Records the bytes and sends them on, dropping viewers that can't keep up
    /// or have gone away so that one bad viewer can't hold up the others.
    pub fn broadcast(&mut self, bytes: &[u8]) {
        self.history.extend(bytes);
        let excess = self.history.len().saturating_sub(self.history_capacity);
        self.history.drain(..excess);

        self.viewers.retain_mut(|viewer| viewer.write_all(bytes).is_ok());
    }

    pub fn viewer_count(&self) -> usize {
        self.viewers.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAPACITY: usize = 8;

    /// A viewer whose received bytes the test can still read after handing it over.
    #[derive(Clone, Default)]
    struct Viewer {
        received: std::rc::Rc<std::cell::RefCell<Vec<u8>>>,
        broken: bool,
    }

    impl Viewer {
        fn text(&self) -> String {
            String::from_utf8(self.received.borrow().clone()).unwrap()
        }
    }

    impl Write for Viewer {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.broken {
                return Err(io::ErrorKind::BrokenPipe.into());
            }
            self.received.borrow_mut().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_new_viewer_gets_an_empty_history_header() {
        let mut hub = Hub::new(CAPACITY);
        let viewer = Viewer::default();
        hub.add_viewer(viewer.clone()).unwrap();
        assert_eq!(viewer.text(), "kitsune-uart history=0\n");
    }

    #[test]
    fn every_viewer_gets_every_byte() {
        let mut hub = Hub::new(CAPACITY);
        let (first, second) = (Viewer::default(), Viewer::default());
        hub.add_viewer(first.clone()).unwrap();
        hub.add_viewer(second.clone()).unwrap();

        hub.broadcast(b"hi");

        assert_eq!(first.text(), "kitsune-uart history=0\nhi");
        assert_eq!(second.text(), "kitsune-uart history=0\nhi");
    }

    #[test]
    fn a_late_viewer_gets_the_history_then_new_bytes() {
        let mut hub = Hub::new(CAPACITY);
        hub.broadcast(b"old");
        let viewer = Viewer::default();
        hub.add_viewer(viewer.clone()).unwrap();

        hub.broadcast(b"new");

        assert_eq!(viewer.text(), "kitsune-uart history=3\noldnew");
    }

    #[test]
    fn history_keeps_only_the_newest_bytes() {
        let mut hub = Hub::new(CAPACITY);
        hub.broadcast(b"0123456");
        hub.broadcast(b"789ab");
        let viewer = Viewer::default();
        hub.add_viewer(viewer.clone()).unwrap();
        assert_eq!(viewer.text(), "kitsune-uart history=8\n456789ab");
    }

    /// Many small writes make the buffer wrap around its storage, so check a
    /// joining viewer's history at every step rather than only at the end.
    #[test]
    fn history_stays_correct_as_the_buffer_wraps() {
        const ROUNDS: u8 = 64;
        let mut hub = Hub::new(CAPACITY);
        let mut sent = Vec::new();

        for byte in b'0'..b'0' + ROUNDS {
            hub.broadcast(&[byte]);
            sent.push(byte);

            let viewer = Viewer::default();
            hub.add_viewer(viewer.clone()).unwrap();

            let newest = &sent[sent.len().saturating_sub(CAPACITY)..];
            let expected = format!("kitsune-uart history={}\n{}", newest.len(), String::from_utf8_lossy(newest));
            assert_eq!(viewer.text(), expected);
        }
    }

    #[test]
    fn a_broken_viewer_is_dropped_without_affecting_the_others() {
        let mut hub = Hub::new(CAPACITY);
        let good = Viewer::default();
        hub.add_viewer(good.clone()).unwrap();
        hub.add_viewer(Viewer::default()).unwrap();
        hub.viewers[1].broken = true;

        hub.broadcast(b"hi");

        assert_eq!(hub.viewer_count(), 1);
        assert_eq!(good.text(), "kitsune-uart history=0\nhi");
    }
}
