//! What the two sides can say to each other. All fields are little-endian.

const TYPE_MOUSE_MOVE: u8 = 1;
const TYPE_KEY: u8 = 2;
const TYPE_UPLOAD_START: u8 = 3;
const TYPE_UPLOAD_CHUNK: u8 = 4;
const TYPE_UPLOAD_COMMIT: u8 = 5;
const TYPE_UPLOAD_REPLY: u8 = 6;
const TYPE_REBOOT: u8 = 7;
const TYPE_INSTALL_FROM_MEMORY: u8 = 8;

/// The most file data a packet carries. It's about 21 ms on the wire at 115200 baud.
pub const CHUNK_BYTES: usize = 240;
const CHUNK_HEADER_BYTES: usize = 5;
pub(crate) const MAX_BYTES: usize = CHUNK_HEADER_BYTES + CHUNK_BYTES;

/// Which file on the card an upload is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// A file that nothing boots from, for testing.
    Scratch,
    /// The file the firmware boots.
    Kernel,
    /// The file the firmware boots once, after `Reboot { trial: true }`.
    Trial,
}

const TARGET_SCRATCH: u8 = 0;
const TARGET_KERNEL: u8 = 1;
const TARGET_TRIAL: u8 = 2;

/// How an upload stands, in the kernel's replies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UploadStatus {
    /// Waiting for the chunk at `next_offset`.
    Receiving,
    /// The file is on the card, and read back correctly.
    Done,
    /// Nothing is in progress, or the start asked for too much, so nothing was touched.
    Refused,
    /// The whole file's CRC was wrong, so the card wasn't touched.
    BadChecksum,
    /// Writing to the card failed, so the file may be partly written.
    WriteFailed,
}

/// What the two sides can say to each other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// Chunks stay inline, as the kernel has no heap to box them on.
#[allow(clippy::large_enum_variant)]
pub enum Message {
    /// How far the mouse moved, in pixels, with positive being right and down. Nothing
    /// acknowledges this: a later move matters more than resending a lost one.
    MouseMove { dx: i16, dy: i16 },
    /// A key pressed as a command. Plain characters on the line are only ever text, as a
    /// lost boundary can let a packet's contents through as characters.
    Key(u8),
    /// Begins an upload of `size` bytes, whose CRC-32 is `crc32`. Replaces any in progress.
    UploadStart { target: Target, size: u32, crc32: u32 },
    /// Writes a file that's already in the upload memory, put there over JTAG, to the card,
    /// if its CRC is right. Answered like `UploadCommit`.
    InstallFromMemory { target: Target, size: u32, crc32: u32 },
    /// Part of the file. The first `length` bytes of `data` belong at `offset`.
    UploadChunk { offset: u32, length: u8, data: [u8; CHUNK_BYTES] },
    /// Checks the whole file, then writes it to the card.
    UploadCommit,
    /// From the kernel, after each of the above three. A host resends if it doesn't arrive.
    UploadReply { next_offset: u32, status: UploadStatus },
    /// Restarts the Pi. A trial boots `Target::Trial` once, and any later reset goes back to
    /// the usual kernel. Nothing answers this, as what follows is the new boot's output.
    Reboot { trial: bool },
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

const fn target_from_code(code: u8) -> Option<Target> {
    match code {
        TARGET_SCRATCH => Some(Target::Scratch),
        TARGET_KERNEL => Some(Target::Kernel),
        TARGET_TRIAL => Some(Target::Trial),
        _ => None,
    }
}

fn write_file_header(output: &mut [u8; MAX_BYTES], kind: u8, target: Target, size: u32, crc32: u32) -> usize {
    output[0] = kind;
    output[1] = match target {
        Target::Scratch => TARGET_SCRATCH,
        Target::Kernel => TARGET_KERNEL,
        Target::Trial => TARGET_TRIAL,
    };
    output[2..6].copy_from_slice(&size.to_le_bytes());
    output[6..10].copy_from_slice(&crc32.to_le_bytes());
    10
}

const fn status_code(status: UploadStatus) -> u8 {
    match status {
        UploadStatus::Receiving => 0,
        UploadStatus::Done => 1,
        UploadStatus::Refused => 2,
        UploadStatus::BadChecksum => 3,
        UploadStatus::WriteFailed => 4,
    }
}

const fn status_from_code(code: u8) -> Option<UploadStatus> {
    match code {
        0 => Some(UploadStatus::Receiving),
        1 => Some(UploadStatus::Done),
        2 => Some(UploadStatus::Refused),
        3 => Some(UploadStatus::BadChecksum),
        4 => Some(UploadStatus::WriteFailed),
        _ => None,
    }
}

impl Message {
    pub(crate) fn to_bytes(self, output: &mut [u8; MAX_BYTES]) -> usize {
        match self {
            Self::MouseMove { dx, dy } => {
                output[0] = TYPE_MOUSE_MOVE;
                output[1..3].copy_from_slice(&dx.to_le_bytes());
                output[3..5].copy_from_slice(&dy.to_le_bytes());
                5
            }
            Self::Key(key) => {
                output[0] = TYPE_KEY;
                output[1] = key;
                2
            }
            Self::UploadStart { target, size, crc32 } => write_file_header(output, TYPE_UPLOAD_START, target, size, crc32),
            Self::InstallFromMemory { target, size, crc32 } => write_file_header(output, TYPE_INSTALL_FROM_MEMORY, target, size, crc32),
            Self::UploadChunk { offset, length, data } => {
                let length = usize::from(length).min(CHUNK_BYTES);
                output[0] = TYPE_UPLOAD_CHUNK;
                output[1..CHUNK_HEADER_BYTES].copy_from_slice(&offset.to_le_bytes());
                output[CHUNK_HEADER_BYTES..CHUNK_HEADER_BYTES + length].copy_from_slice(&data[..length]);
                CHUNK_HEADER_BYTES + length
            }
            Self::UploadCommit => {
                output[0] = TYPE_UPLOAD_COMMIT;
                1
            }
            Self::UploadReply { next_offset, status } => {
                output[0] = TYPE_UPLOAD_REPLY;
                output[1..5].copy_from_slice(&next_offset.to_le_bytes());
                output[5] = status_code(status);
                6
            }
            Self::Reboot { trial } => {
                output[0] = TYPE_REBOOT;
                output[1] = u8::from(trial);
                2
            }
        }
    }

    pub(crate) fn from_bytes(bytes: &[u8]) -> Option<Self> {
        match *bytes {
            [TYPE_MOUSE_MOVE, dx0, dx1, dy0, dy1] => {
                Some(Self::MouseMove { dx: i16::from_le_bytes([dx0, dx1]), dy: i16::from_le_bytes([dy0, dy1]) })
            }
            [TYPE_KEY, key] => Some(Self::Key(key)),
            [TYPE_UPLOAD_START, target, _, _, _, _, _, _, _, _] => {
                Some(Self::UploadStart { target: target_from_code(target)?, size: u32_at(bytes, 2), crc32: u32_at(bytes, 6) })
            }
            [TYPE_INSTALL_FROM_MEMORY, target, _, _, _, _, _, _, _, _] => {
                Some(Self::InstallFromMemory { target: target_from_code(target)?, size: u32_at(bytes, 2), crc32: u32_at(bytes, 6) })
            }
            [TYPE_UPLOAD_CHUNK, _, _, _, _, ref rest @ ..] if !rest.is_empty() && rest.len() <= CHUNK_BYTES => {
                let mut data = [0; CHUNK_BYTES];
                data[..rest.len()].copy_from_slice(rest);
                Some(Self::UploadChunk { offset: u32_at(bytes, 1), length: rest.len() as u8, data })
            }
            [TYPE_UPLOAD_COMMIT] => Some(Self::UploadCommit),
            [TYPE_UPLOAD_REPLY, _, _, _, _, code] => {
                Some(Self::UploadReply { next_offset: u32_at(bytes, 1), status: status_from_code(code)? })
            }
            [TYPE_REBOOT, 0] => Some(Self::Reboot { trial: false }),
            [TYPE_REBOOT, 1] => Some(Self::Reboot { trial: true }),
            _ => None,
        }
    }
}
