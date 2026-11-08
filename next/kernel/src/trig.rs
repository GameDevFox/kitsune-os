//! Cosines and sines in fixed point, from a table built at compile time, as the kernel
//! has no floating point. The turning logo and the test tone use them.
//! `tools/spin_model.py` repeats the table exactly, so keep the two in step.

/// Steps in a whole turn, for the cosine table.
pub const STEPS: usize = 1024;
const QUARTER: usize = STEPS / 4;
/// Cosines are kept with this many fractional bits.
pub const FRACTION_BITS: u32 = 16;
pub const ONE: i64 = 1 << FRACTION_BITS;

// `cos_fixed`'s own fixed point: 32 fractional bits, and pi to that precision.
const TABLE_BITS: u32 = 32;
const PI: i128 = 13_493_037_705;
/// Taylor terms. Up to pi, the first left out is below 1/100,000.
const TERMS: i128 = 8;

/// The cosine of `step` steps of `STEPS` to a turn, with `FRACTION_BITS` fractional bits,
/// by its Taylor series, at compile time.
const fn cos_fixed(step: usize) -> i32 {
    // Into -pi to pi, where the series converges quickly.
    let mut angle = 2 * PI * step as i128 / STEPS as i128;
    if angle > PI {
        angle -= 2 * PI;
    }
    let squared = (angle * angle) >> TABLE_BITS;
    let mut term: i128 = 1 << TABLE_BITS;
    let mut sum = term;
    let mut n = 1;
    while n <= TERMS {
        term = -((term * squared) >> TABLE_BITS) / ((2 * n - 1) * (2 * n));
        sum += term;
        n += 1;
    }
    (sum >> (TABLE_BITS - FRACTION_BITS)) as i32
}

const COSINES: [i32; STEPS] = {
    let mut table = [0; STEPS];
    let mut step = 0;
    while step < STEPS {
        table[step] = cos_fixed(step);
        step += 1;
    }
    table
};

/// Within a few parts in 100,000 of the exact value.
const TOLERANCE: i32 = 2;
const fn near(value: i32, want: i64) -> bool {
    (value as i64 - want).abs() <= TOLERANCE as i64
}
const _: () = assert!(near(COSINES[0], ONE));
const _: () = assert!(near(COSINES[QUARTER], 0));
const _: () = assert!(near(COSINES[2 * QUARTER], -ONE));
const _: () = assert!(near(COSINES[3 * QUARTER], 0));
// Near 60 degrees, where the cosine is near a half. STEPS / 6 isn't a whole step, so
// this checks step 171, 60.12 degrees, whose cosine is 0.49823.
const _: () = assert!(near(COSINES[171], 32_652));

/// The cosine of `step` steps of `STEPS` to a turn, as a fraction of `ONE`.
pub fn cos(step: i64) -> i64 {
    COSINES[step.rem_euclid(STEPS as i64) as usize] as i64
}

/// The sine of `step` steps of `STEPS` to a turn, as a fraction of `ONE`.
pub fn sin(step: i64) -> i64 {
    cos(step - QUARTER as i64)
}
