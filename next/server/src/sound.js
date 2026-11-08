// Plays the Pi's headphone jack in the page, as the server records it from this PC's
// line-in. `/audio` sends raw 48 kHz stereo 16-bit little-endian samples, which must
// match `audio.rs`. Each piece is scheduled to play straight after the one before.

export const SAMPLE_RATE = 48000;
export const CHANNELS = 2;
const BYTES_PER_SAMPLE = 2;
export const FRAME_BYTES = CHANNELS * BYTES_PER_SAMPLE;
const FULL_SCALE = 32768;
// How far ahead of now playback starts, to ride out uneven arrival over the network.
export const LEAD_SECONDS = 0.08;
// Queued sound this far ahead means pieces bunched up, so later ones are dropped until
// it shrinks, keeping sound close to the video rather than ever further behind.
export const MAX_AHEAD_SECONDS = 0.4;

/** Converts interleaved 16-bit samples to one array of -1 to 1 per channel. */
export const toChannels = (bytes) => {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const frames = Math.floor(bytes.byteLength / FRAME_BYTES);
  const channels = Array.from({ length: CHANNELS }, () => new Float32Array(frames));
  for (let frame = 0; frame < frames; frame++) {
    for (let channel = 0; channel < CHANNELS; channel++) {
      channels[channel][frame] = view.getInt16(frame * FRAME_BYTES + channel * BYTES_PER_SAMPLE, true) / FULL_SCALE;
    }
  }
  return channels;
};

/** Turns network reads, which can end mid-frame, into whole frames, keeping the rest for next time. */
export const FrameReader = () => {
  let carry = new Uint8Array(0);
  return {
    push: (bytes) => {
      const joined = new Uint8Array(carry.length + bytes.length);
      joined.set(carry);
      joined.set(bytes, carry.length);
      const whole = joined.length - (joined.length % FRAME_BYTES);
      carry = joined.slice(whole);
      return toChannels(joined.subarray(0, whole));
    },
  };
};

/** Decides when each piece starts: back to back, a little ahead of now after a gap,
 * or `null` to drop it if too much is already queued. */
export const Schedule = () => {
  let next = 0;
  return {
    place: (now, duration) => {
      if (next < now) next = now + LEAD_SECONDS;
      if (next - now > MAX_AHEAD_SECONDS) return null;
      const start = next;
      next += duration;
      return start;
    },
  };
};

let playing = null;

/** Plays everything that arrives, until the stream ends or is stopped. */
const listen = async (response, context, stop, report) => {
  try {
    const frames = FrameReader();
    const schedule = Schedule();
    const stream = response.body.getReader();
    for (;;) {
      const { value, done } = await stream.read();
      if (done) break;
      const channels = frames.push(value);
      if (channels[0].length === 0) continue;
      const buffer = context.createBuffer(CHANNELS, channels[0].length, SAMPLE_RATE);
      channels.forEach((data, channel) => buffer.copyToChannel(data, channel));
      const start = schedule.place(context.currentTime, buffer.duration);
      if (start === null) continue;
      const source = context.createBufferSource();
      source.buffer = buffer;
      source.connect(context.destination);
      source.start(start);
    }
    report("sound stopped: the server has none", false);
  } catch (error) {
    if (!stop.signal.aborted) report(`sound stopped: ${error.message}`, false);
  } finally {
    if (playing?.stop === stop) playing = null;
    context.close();
  }
};

/** Connects to the server's sound, and returns a promise that settles once it's
 * listening, or has failed. The playing goes on after that. */
const play = (report) => {
  const context = new AudioContext({ sampleRate: SAMPLE_RATE });
  const stop = new AbortController();
  playing = { context, stop, ready: null };
  playing.ready = (async () => {
    try {
      const response = await fetch("/audio", { signal: stop.signal });
      if (!response.ok) throw new Error(`the server said ${response.status}`);
      report("sound on", true);
      listen(response, context, stop, report);
    } catch (error) {
      if (playing?.stop === stop) playing = null;
      context.close();
      if (!stop.signal.aborted) report(`sound stopped: ${error.message}`, false);
    }
  })();
  return playing.ready;
};

/** Turns sound on or off. A browser only allows it after a key or click, which this
 * must be called from. `report(text, fades)` shows how it went. */
export const toggleSound = (report) => {
  if (!playing) return play(report);
  playing.stop.abort();
  playing = null;
  report("sound off", true);
  return Promise.resolve();
};

/** Turns sound on if it's off, as `toggleSound` does, and returns a promise that settles
 * once it's listening, so that what follows can be heard from its start. */
export const startSound = (report) => (playing ? playing.ready : play(report));
