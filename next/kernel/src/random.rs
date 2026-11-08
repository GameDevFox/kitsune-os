//! Random numbers from the hardware generator, with a quick check that they look random.

use crate::board::{self, rng::Rng};
use crate::devices;

/// Enough words for the counts below to mean something, and quick to gather.
const SAMPLE_WORDS: usize = 1024;
const BITS_PER_WORD: u32 = 32;
/// Give up waiting for a word after this long: the generator makes about 1 Mbit/s.
const WAIT_MICROSECONDS: u32 = 100_000;
const PERCENT: u64 = 100;
const TENTHS: u64 = 10;

/// The generator, if the device tree has one. An optional device, so there's no fallback.
fn rng() -> Option<Rng> {
    Some(Rng::new(devices::address_of("random").or_else(|| devices::address_of("rng"))?))
}

/// Starts the generator, then gathers words and prints how random they look: about half
/// their bits set, and no word repeated.
pub fn check() {
    let Some(rng) = rng() else {
        println!("Random: no generator in the device tree, so skipped.");
        return;
    };
    rng.start();
    let mut words = [0u32; SAMPLE_WORDS];
    let start = board::microseconds();
    for slot in words.iter_mut() {
        let waiting = board::microseconds();
        *slot = loop {
            if let Some(word) = rng.try_next() {
                break word;
            }
            if board::microseconds().wrapping_sub(waiting) > WAIT_MICROSECONDS {
                println!("Random: the generator stopped giving words.");
                return;
            }
        };
    }
    let elapsed = board::microseconds().wrapping_sub(start);
    let ones: u64 = words.iter().map(|word| u64::from(word.count_ones())).sum();
    let bits = SAMPLE_WORDS as u64 * u64::from(BITS_PER_WORD);
    let mut sorted = words;
    sorted.sort_unstable();
    let repeats = sorted.windows(2).filter(|pair| pair[0] == pair[1]).count();
    let tenths_of_a_percent = ones * PERCENT * TENTHS / bits;
    println!(
        "Random: {SAMPLE_WORDS} words in {elapsed} us, {}.{}% of bits set, {repeats} repeated, first {:#010x}.",
        tenths_of_a_percent / TENTHS,
        tenths_of_a_percent % TENTHS,
        words[0]
    );
}
