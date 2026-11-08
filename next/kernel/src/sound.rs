//! Sound played in the background: a test tone, and a recorded clip from the sound
//! region. The CPU writes the whole sound out first, then DMA plays it while the kernel
//! carries on, and `Player::poll` notices when it has finished.

use crate::assets::{self, Assets};
use crate::board::{self, audio};
use crate::trig::{sin, ONE, STEPS};

/// Plays the test tone, which differs per side so a recording can tell them apart.
pub const TONE_KEY: u8 = b'7';
/// Plays the clip in the sound region, which `tools/voice.py` loads over JTAG, or else the
/// one stored with the images, which survives a power cycle.
pub const CLIP_KEY: u8 = b'8';

/// A sine wave on each side, for a time, with pitches in hundredths of a hertz.
pub struct Tone {
    pub left_centihertz: u32,
    pub right_centihertz: u32,
    pub milliseconds: u32,
}

/// Middle C on the right, and the G above it on the left: a fifth apart, so they sound
/// pleasant together, and the sides can be told apart.
pub const TEST_TONE: Tone = Tone { left_centihertz: 39_200, right_centihertz: 26_163, milliseconds: 500 };
const CENTIHERTZ_PER_HERTZ: u64 = 100;
/// The tone fades in and out over this long, so it doesn't start or stop with a click.
const FADE_MILLISECONDS: u32 = 20;

const MILLISECONDS_PER_SECOND: u32 = 1000;
/// A quarter of full scale either side of silence: loud enough, without being harsh.
const AMPLITUDE: u32 = audio::MAX_SAMPLE / 4;
const SILENCE: u32 = audio::MAX_SAMPLE / 2;
/// The output rests at 0 V when off, but silence is half of full scale, so jumping
/// between them makes a pop. Easing between them over this long avoids it.
const RAMP_MILLISECONDS: u32 = 50;

// A clip, as `tools/voice.py` writes it, little-endian: the magic bytes, its sample
// rate, its number of samples, then that many signed 16-bit mono samples.
const CLIP_MAGIC: &[u8] = b"KITSND01";
const CLIP_RATE_AT: usize = 8;
const CLIP_COUNT_AT: usize = 12;
const CLIP_HEADER_BYTES: usize = 16;
const CLIP_SAMPLE_BYTES: usize = 2;
const WORD_BYTES: usize = 4;
/// A 16-bit sample's range either side of zero.
const CLIP_FULL_SCALE: i32 = 32768;

/// Plays sounds, and turns the PWM off once each has finished.
pub struct Player {
    playing: bool,
}

impl Player {
    pub fn new() -> Self {
        Self { playing: false }
    }

    /// Starts the sound for a key that makes one, cutting off any already playing.
    /// Returns false for any other key.
    pub fn handle_key(&mut self, key: u8, assets: &mut Option<Assets>) -> bool {
        let started = match key {
            TONE_KEY => play_tone(&TEST_TONE),
            CLIP_KEY => play_clip(choose_clip(assets)),
            _ => return false,
        };
        self.playing |= started;
        true
    }

    /// Call often. Stops the PWM once the sound has finished, and reports any trouble.
    pub fn poll(&mut self) {
        if !self.playing || audio::playing() {
            return;
        }
        self.playing = false;
        if let Some((dma_status, dma_debug, pwm_status)) = audio::problems() {
            println!("Sound problems: DMA status {dma_status:#x}, debug {dma_debug:#x}, PWM status {pwm_status:#x}.");
        }
        audio::stop();
    }
}

/// The sample at a point in a sine wave, at `loudness` out of `full`. The angle comes
/// from the sample's own number, rounded to the table's nearest step, so no error builds.
fn sine(sample: u32, centihertz: u32, loudness: u32, full: u32) -> u32 {
    let cycles_scale = CENTIHERTZ_PER_HERTZ * u64::from(audio::SAMPLE_RATE);
    let step = (u64::from(sample) * u64::from(centihertz) * STEPS as u64 + cycles_scale / 2) / cycles_scale;
    let offset = i64::from(AMPLITUDE) * sin(step as i64) * i64::from(loudness) / (ONE * i64::from(full));
    (i64::from(SILENCE) + offset) as u32
}

/// Eases both sides from one level to another.
fn ramp(samples: &mut audio::Samples, from: u32, to: u32) -> bool {
    let steps = audio::SAMPLE_RATE / MILLISECONDS_PER_SECOND * RAMP_MILLISECONDS;
    (0..steps).all(|step| {
        let level = (u64::from(from) * u64::from(steps - step) + u64::from(to) * u64::from(step)) / u64::from(steps);
        samples.push(level as u32, level as u32)
    })
}

/// Starts playing `count` samples in the background, asking `sample_at` for each one's
/// left and right level. Returns whether it started.
fn play(count: u32, sample_at: impl Fn(u32) -> (u32, u32)) -> bool {
    if !audio::available() {
        println!("No sound: GPIO 40 and 41 aren't connected to the PWM.");
        return false;
    }
    let start = board::microseconds();
    let mut samples = audio::Samples::new();
    let fits = ramp(&mut samples, 0, SILENCE)
        && (0..count).all(|sample| {
            let (left, right) = sample_at(sample);
            samples.push(left, right)
        })
        && ramp(&mut samples, SILENCE, 0);
    if !fits {
        println!("The sound is longer than the DMA buffer, so it will be cut short.");
    }
    let elapsed = board::microseconds().wrapping_sub(start);
    println!("Sound prepared in {elapsed} us; playing in the background.");
    audio::play(samples);
    true
}

fn play_tone(tone: &Tone) -> bool {
    let per_millisecond = audio::SAMPLE_RATE / MILLISECONDS_PER_SECOND;
    let count = per_millisecond * tone.milliseconds;
    let fade = per_millisecond * FADE_MILLISECONDS;
    play(count, |sample| {
        let loudness = sample.min(count - sample).min(fade);
        (sine(sample, tone.left_centihertz, loudness, fade), sine(sample, tone.right_centihertz, loudness, fade))
    })
}

fn read_u32(data: &[u8], at: usize) -> Option<u32> {
    let bytes = data.get(at..at + WORD_BYTES)?;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

/// A clip loaded over JTAG, as that's how to try another, or else the images' own.
fn choose_clip(assets: &mut Option<Assets>) -> &'static [u8] {
    let loaded = board::sound_region();
    if is_clip(loaded) {
        return loaded;
    }
    assets.as_mut().and_then(|assets| assets.get(assets::VOICE)).unwrap_or(&[])
}

fn is_clip(data: &[u8]) -> bool {
    data.get(..CLIP_MAGIC.len()) == Some(CLIP_MAGIC)
}

/// Starts a clip on both sides, if `data` starts with one recorded at the PWM's rate.
/// Returns whether it started.
fn play_clip(data: &[u8]) -> bool {
    if !is_clip(data) {
        println!("No clip, loaded or on the card: tools/voice.py loads one.");
        return false;
    }
    let (Some(rate), Some(count)) = (read_u32(data, CLIP_RATE_AT), read_u32(data, CLIP_COUNT_AT)) else {
        return false;
    };
    if rate != audio::SAMPLE_RATE {
        println!("The clip is at {rate} samples a second, but the PWM plays {}.", audio::SAMPLE_RATE);
        return false;
    }
    let Some(samples) = data.get(CLIP_HEADER_BYTES..CLIP_HEADER_BYTES + count as usize * CLIP_SAMPLE_BYTES) else {
        println!("The clip says it has {count} samples, more than the sound region holds.");
        return false;
    };
    // Full scale either side of silence, as a recording sets its own loudness.
    let half_range = SILENCE as i32;
    play(count, |sample| {
        let at = sample as usize * CLIP_SAMPLE_BYTES;
        let value = i32::from(i16::from_le_bytes([samples[at], samples[at + 1]]));
        let level = (half_range + value * half_range / CLIP_FULL_SCALE) as u32;
        (level, level)
    })
}
