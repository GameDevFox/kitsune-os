// Tests the sound player's decoding and timing, which need no browser.
import { test } from "node:test";
import assert from "node:assert/strict";

const SOUND = process.env.SOUND_JS ?? new URL("../../server/src/sound.js", import.meta.url).href;
const { toChannels, FrameReader, Schedule, FRAME_BYTES, LEAD_SECONDS, MAX_AHEAD_SECONDS } = await import(SOUND);

const FULL_SCALE = 32768;
/** Interleaved left and right 16-bit little-endian samples, as the server sends them. */
const pcm = (pairs) => {
  const view = new DataView(new ArrayBuffer(pairs.length * FRAME_BYTES));
  pairs.forEach(([left, right], frame) => {
    view.setInt16(frame * FRAME_BYTES, left, true);
    view.setInt16(frame * FRAME_BYTES + FRAME_BYTES / 2, right, true);
  });
  return new Uint8Array(view.buffer);
};
const PAIRS = [[0, 1000], [-32768, 32767], [16384, -16384]];

test("samples become left and right, scaled to -1 to 1", () => {
  const [left, right] = toChannels(pcm(PAIRS));
  assert.deepEqual(Array.from(left), PAIRS.map(([value]) => value / FULL_SCALE));
  assert.deepEqual(Array.from(right), PAIRS.map(([, value]) => value / FULL_SCALE));
});

test("reads split anywhere still give every frame once, in order", () => {
  const bytes = pcm(PAIRS);
  for (let split = 0; split <= bytes.length; split++) {
    const reader = FrameReader();
    const parts = [reader.push(bytes.subarray(0, split)), reader.push(bytes.subarray(split))];
    const left = parts.flatMap(([channel]) => Array.from(channel));
    const right = parts.flatMap(([, channel]) => Array.from(channel));
    assert.deepEqual(left, PAIRS.map(([value]) => value / FULL_SCALE), `split at ${split}`);
    assert.deepEqual(right, PAIRS.map(([, value]) => value / FULL_SCALE), `split at ${split}`);
  }
});

const PIECE = 0.02;

test("pieces arriving steadily play back to back", () => {
  const schedule = Schedule();
  const first = schedule.place(10, PIECE);
  assert.equal(first, 10 + LEAD_SECONDS);
  assert.equal(schedule.place(10 + PIECE, PIECE), first + PIECE);
  assert.equal(schedule.place(10 + 2 * PIECE, PIECE), first + PIECE + PIECE);
});

test("after a gap, playback starts a little ahead of now again", () => {
  const schedule = Schedule();
  schedule.place(10, PIECE);
  assert.equal(schedule.place(20, PIECE), 20 + LEAD_SECONDS);
});

test("pieces that bunch up are dropped until the queue shrinks", () => {
  const schedule = Schedule();
  const placed = [];
  for (let piece = 0; piece < 100; piece++) placed.push(schedule.place(10, PIECE));
  const kept = placed.filter((start) => start !== null);
  assert.ok(placed.includes(null), "nothing was dropped");
  assert.ok(Math.max(...kept) - 10 <= MAX_AHEAD_SECONDS, `queued ${Math.max(...kept) - 10} s ahead`);
  // Once time has moved on, pieces are taken again.
  assert.notEqual(schedule.place(10 + MAX_AHEAD_SECONDS, PIECE), null);
});
