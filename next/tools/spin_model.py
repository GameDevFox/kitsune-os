"""The turning logo, drawn with exactly the integer arithmetic `kernel/src/spin.rs` uses,
so a screen from QEMU can be compared pixel for pixel. Keep the two in step.

Rust's `/` rounds toward zero, where Python's `//` rounds down, so `divide` stands in for
it. Both shift right rounding down.
"""

import numpy as np

STEPS = 1024
QUARTER = STEPS // 4
FRACTION_BITS = 16
ONE = 1 << FRACTION_BITS
TURN_FRAMES = 180
VIEW_DISTANCE = 1000
EXTRA_ROWS = 12
TABLE_BITS = 32
PI = 13_493_037_705
TERMS = 8
WIDTH, HEIGHT = 168, 256
DRAWN_ROWS = HEIGHT + 2 * EXTRA_ROWS


def divide(a, b):
    quotient = abs(a) // abs(b)
    return quotient if (a < 0) == (b < 0) else -quotient


def cos_fixed(step):
    angle = divide(2 * PI * step, STEPS)
    if angle > PI:
        angle -= 2 * PI
    squared = (angle * angle) >> TABLE_BITS
    term = 1 << TABLE_BITS
    total = term
    for n in range(1, TERMS + 1):
        term = divide(-((term * squared) >> TABLE_BITS), (2 * n - 1) * (2 * n))
        total += term
    return total >> (TABLE_BITS - FRACTION_BITS)


COSINES = [cos_fixed(step) for step in range(STEPS)]


def cos(step):
    return COSINES[step % STEPS]


def sources(angle):
    """Each column's source column and depth, or (-1, 0) where nothing lands."""
    cosine, sine = cos(angle), cos(angle - QUARTER)
    columns, depths = np.full(WIDTH, -1, dtype=np.int64), np.zeros(WIDTH, dtype=np.int64)
    for column in range(WIDTH):
        from_center = 2 * column + 1 - WIDTH
        below = 2 * VIEW_DISTANCE * cosine - from_center * sine
        if below == 0:
            continue
        unsqueezed = divide(from_center * 2 * VIEW_DISTANCE * ONE, below)
        if abs(unsqueezed) < WIDTH:
            depths[column] = ONE + divide(unsqueezed * sine, 2 * VIEW_DISTANCE)
            columns[column] = (unsqueezed + WIDTH) // 2
    return columns, depths


def render(original, frame):
    """The box, `DRAWN_ROWS` by `WIDTH`, as the kernel leaves it on `frame` of the turn.
    `original` is the logo, `HEIGHT` by `WIDTH` by 3, black where it's clear."""
    columns, depths = sources(STEPS * frame // TURN_FRAMES)
    from_center = 2 * np.arange(DRAWN_ROWS, dtype=np.int64) + 1 - DRAWN_ROWS
    unscaled = (from_center[:, None] * depths[None, :]) >> FRACTION_BITS
    lands = (np.abs(unscaled) < HEIGHT) & (columns[None, :] >= 0)
    # Every value here is positive, so rounding down matches Rust's division.
    rows = np.where(lands, (unscaled + HEIGHT) // 2, 0)
    picture = original[rows, np.maximum(columns, 0)[None, :]]
    picture[~lands] = 0
    return picture


def all_frames(original):
    """Each frame of the turn, keyed by its frame number."""
    return {frame: render(original, frame) for frame in range(TURN_FRAMES + 1)}
