//! Receives a file over serial into RAM, and only then, once its CRC is right, writes it
//! to the card. See `tools/upload`, which sends it, for the other half.

use core::cell::UnsafeCell;

use protocol::{crc32, Message, Target, UploadStatus, MAX_PACKET_BYTES};

use crate::board;
use crate::card;
use crate::watchdog;

/// The size of the files it can write, which `scripts/card.py` makes them.
const MAX_FILE_BYTES: usize = 512 * 1024;
/// How long a host may go quiet before an upload is given up on, so that the screen resumes.
const STALL_MICROSECONDS: u32 = 5_000_000;
const MICROSECONDS_PER_MILLISECOND: u32 = 1000;

const fn file_name(target: Target) -> &'static str {
    match target {
        Target::Scratch => "update-test.bin",
        Target::Kernel => "kitsune-next.img",
        Target::Trial => "kitsune-try.img",
    }
}

#[derive(Clone, Copy)]
struct Upload {
    target: Target,
    size: u32,
    crc32: u32,
    next_offset: u32,
    last_heard: u32,
}

struct State {
    upload: Option<Upload>,
    /// The last commit's answer, in case the reply was lost and it gets asked again.
    finished: Option<UploadStatus>,
}

struct Shared(UnsafeCell<State>);

// SAFETY: the kernel runs on one core with interrupts masked, so nothing can use
// this at the same time. That must change if either of those does.
unsafe impl Sync for Shared {}

static STATE: Shared = Shared(UnsafeCell::new(State { upload: None, finished: None }));

fn state() -> &'static mut State {
    // SAFETY: see `Shared`, and no caller keeps this across another call to it.
    unsafe { &mut *STATE.0.get() }
}

fn reply(next_offset: u32, status: UploadStatus) {
    let mut packet = [0; MAX_PACKET_BYTES];
    let length = Message::UploadReply { next_offset, status }.encode(&mut packet);
    board::console_write(&packet[..length]);
}

/// Whether an upload is under way, during which nothing but serial should get attention,
/// as its small buffer overflows in about 3 ms at this rate.
pub fn is_active() -> bool {
    state().upload.is_some()
}

/// Gives up on an upload if its host has gone quiet.
pub fn give_up_if_stalled() {
    let state = state();
    if state.upload.is_some_and(|upload| board::microseconds().wrapping_sub(upload.last_heard) > STALL_MICROSECONDS) {
        println!("Upload: nothing heard for a while, so gave up. The card wasn't touched.");
        state.upload = None;
    }
}

fn commit(upload: Upload, size: usize) -> UploadStatus {
    let data = &board::upload_memory()[..size];
    if crc32(data) != upload.crc32 {
        println!("Upload: the checksum was wrong, so the card wasn't touched.");
        return UploadStatus::BadChecksum;
    }
    let start = board::microseconds();
    let result = card::overwrite(file_name(upload.target), data);
    let elapsed = board::microseconds().wrapping_sub(start) / MICROSECONDS_PER_MILLISECOND;
    match result {
        Ok(()) => {
            println!("Upload: wrote {size} bytes to {} in {elapsed} ms.", file_name(upload.target));
            UploadStatus::Done
        }
        Err(error) => {
            println!("Upload: writing to {} failed: {error:?}", file_name(upload.target));
            UploadStatus::WriteFailed
        }
    }
}

/// Handles an upload message, and always answers it.
pub fn handle(message: &Message) {
    let state = state();
    match *message {
        Message::UploadStart { target, size, crc32 } => {
            state.finished = None;
            if size == 0 || size as usize > MAX_FILE_BYTES {
                state.upload = None;
                reply(0, UploadStatus::Refused);
                return;
            }
            state.upload = Some(Upload { target, size, crc32, next_offset: 0, last_heard: board::microseconds() });
            reply(0, UploadStatus::Receiving);
        }
        Message::InstallFromMemory { target, size, crc32 } => {
            state.upload = None;
            if size == 0 || size as usize > MAX_FILE_BYTES {
                state.finished = Some(UploadStatus::Refused);
                reply(0, UploadStatus::Refused);
                return;
            }
            let upload = Upload { target, size, crc32, next_offset: size, last_heard: board::microseconds() };
            let status = commit(upload, size as usize);
            self::state().finished = Some(status);
            reply(size, status);
        }
        Message::UploadChunk { offset, length, ref data } => {
            let Some(upload) = state.upload.as_mut() else {
                reply(0, UploadStatus::Refused);
                return;
            };
            upload.last_heard = board::microseconds();
            let end = offset.saturating_add(u32::from(length));
            // Anything else is a resend of something already here, or out of order. Either
            // way, the reply says what's actually wanted next.
            if offset == upload.next_offset && end <= upload.size {
                board::upload_memory()[offset as usize..end as usize].copy_from_slice(&data[..usize::from(length)]);
                upload.next_offset = end;
            }
            reply(upload.next_offset, UploadStatus::Receiving);
        }
        Message::UploadCommit => {
            let Some(mut upload) = state.upload else {
                reply(0, state.finished.unwrap_or(UploadStatus::Refused));
                return;
            };
            upload.last_heard = board::microseconds();
            if upload.next_offset != upload.size {
                reply(upload.next_offset, UploadStatus::Receiving);
                return;
            }
            let status = commit(upload, upload.size as usize);
            let state = self::state();
            state.upload = None;
            state.finished = Some(status);
            reply(upload.size, status);
        }
        Message::Reboot { trial } => {
            println!("Rebooting{}.", if trial { " into the trial kernel" } else { "" });
            watchdog::reboot(trial);
        }
        Message::MouseMove { .. } | Message::Key(_) | Message::UploadReply { .. } => {}
    }
}
