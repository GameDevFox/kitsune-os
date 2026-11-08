//! `print!` and `println!`, which `std` would normally provide.

use core::cell::UnsafeCell;
use core::fmt::{self, Write};

use protocol::{Message, Received, Receiver};

use crate::board;
use crate::text::TextScreen;
use crate::video::Framebuffer;

/// How many echoed bytes can wait to be drawn. Beyond this they still go back over
/// serial, but don't reach the screen.
const PENDING_CAPACITY: usize = 8192;

/// How long a packet may pause before it's given up on. A stray zero byte also looks like
/// the start of one, which shouldn't hold up typing for long.
const PACKET_TIMEOUT_MICROSECONDS: u32 = 20_000;

/// Bytes waiting to be drawn, oldest first.
struct Pending {
    bytes: [u8; PENDING_CAPACITY],
    start: usize,
    length: usize,
}

impl Pending {
    fn push(&mut self, byte: u8) {
        if self.length < PENDING_CAPACITY {
            self.bytes[(self.start + self.length) % PENDING_CAPACITY] = byte;
            self.length += 1;
        }
    }

    fn pop(&mut self) -> Option<u8> {
        if self.length == 0 {
            return None;
        }
        let byte = self.bytes[self.start];
        self.start = (self.start + 1) % PENDING_CAPACITY;
        self.length -= 1;
        Some(byte)
    }
}

struct State {
    /// The screen text also goes to, once there is one.
    screen: Option<TextScreen>,
    pending: Pending,
    /// The newest key command received that nothing has asked for yet.
    latest_key: Option<u8>,
    receiver: Receiver,
    last_received: u32,
    /// Mouse movement that nothing has asked for yet.
    mouse: (i32, i32),
}

struct Shared(UnsafeCell<State>);

// SAFETY: the kernel runs on one core with interrupts masked, so nothing can use
// this at the same time. That must change if either of those does.
unsafe impl Sync for Shared {}

static STATE: Shared = Shared(UnsafeCell::new(State {
    screen: None,
    pending: Pending { bytes: [0; PENDING_CAPACITY], start: 0, length: 0 },
    latest_key: None,
    receiver: Receiver::new(),
    last_received: 0,
    mouse: (0, 0),
}));

fn state() -> &'static mut State {
    // SAFETY: see `Shared`, and no caller keeps this across another call to it.
    unsafe { &mut *STATE.0.get() }
}

/// From now on, everything printed also appears on this framebuffer.
pub fn attach_screen(framebuffer: Framebuffer) {
    state().screen = Some(TextScreen::new(framebuffer));
}

fn draw(bytes: &[u8]) {
    if let Some(screen) = state().screen.as_mut() {
        screen.write(bytes);
    }
}

fn write_all(bytes: &[u8]) {
    board::console_write(bytes);
    draw(bytes);
}

struct Console;

impl Write for Console {
    /// Serial terminals need `\r\n` to return to the start of the next line.
    fn write_str(&mut self, text: &str) -> fmt::Result {
        for (index, line) in text.split('\n').enumerate() {
            if index > 0 {
                write_all(b"\r\n");
            }
            write_all(line.as_bytes());
        }
        Ok(())
    }
}

/// Writes to serial only, for output too long for the screen.
pub struct Serial;

impl Write for Serial {
    /// Serial terminals need `\r\n` to return to the start of the next line.
    fn write_str(&mut self, text: &str) -> fmt::Result {
        for (index, line) in text.split('\n').enumerate() {
            if index > 0 {
                board::console_write(b"\r\n");
            }
            board::console_write(line.as_bytes());
        }
        Ok(())
    }
}

pub fn print_args(args: fmt::Arguments) {
    // `Console` cannot fail, so there is no error to handle.
    let _ = Console.write_fmt(args);
}

/// Returns the newest key command received since this was last called, if any.
pub fn take_latest_key() -> Option<u8> {
    state().latest_key.take()
}

/// Returns the mouse movement received since this was last called.
pub fn take_mouse() -> (i32, i32) {
    core::mem::take(&mut state().mouse)
}

fn receive(byte: u8) {
    let state = state();
    state.last_received = board::microseconds();
    match state.receiver.push(byte) {
        Received::Character(character) => echo(character),
        Received::Absorbed => {}
        Received::Message(Message::MouseMove { dx, dy }) => {
            state.mouse.0 = state.mouse.0.saturating_add(i32::from(dx));
            state.mouse.1 = state.mouse.1.saturating_add(i32::from(dy));
        }
        Received::Message(Message::Key(key)) => state.latest_key = Some(key),
        Received::Message(message) => crate::upload::handle(&message),
    }
}

fn abandon_stalled_packet() {
    let state = state();
    if state.receiver.in_packet() && board::microseconds().wrapping_sub(state.last_received) > PACKET_TIMEOUT_MICROSECONDS {
        state.receiver.abandon();
    }
}

fn echo(byte: u8) {
    let bytes: &[u8] = match byte {
        // Enter starts a new line, whichever line ending it was sent as.
        b'\r' | b'\n' => b"\r\n",
        _ => &[byte],
    };
    board::console_write(bytes);
    if state().screen.is_some() {
        for &byte in bytes {
            state().pending.push(byte);
        }
    }
}

/// Echoes whatever has arrived and draws anything waiting, without blocking on
/// input. Drawing a character takes far longer than receiving one, so the UART
/// is emptied again between characters, as its small FIFO would otherwise
/// overflow and lose bytes. The screen can lag behind during a burst.
pub fn echo_pending_input() {
    loop {
        while let Some(byte) = board::console_try_read() {
            receive(byte);
        }
        abandon_stalled_packet();
        let state = state();
        if state.screen.as_mut().is_some_and(TextScreen::clear_step) {
            continue;
        }
        let Some(byte) = state.pending.pop() else {
            return;
        };
        draw(&[byte]);
    }
}

macro_rules! print {
    ($($arg:tt)*) => {
        $crate::console::print_args(format_args!($($arg)*))
    };
}

macro_rules! println {
    () => {
        print!("\n")
    };
    ($($arg:tt)*) => {{
        print!($($arg)*);
        print!("\n");
    }};
}
