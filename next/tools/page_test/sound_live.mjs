// Checks the real page's sound: opens it in a throwaway headless browser, muted so
// nothing reaches the speakers, presses s, and reports what the player schedules. With
// --tone it also presses 7 on the Pi, to hear its test tone come through, and then 7 on
// the page with sound off, which should turn it on. Needs `cargo server` running.
// Usage: node tools/page_test/sound_live.mjs [--tone]
import { spawn } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const TONE = process.argv.includes("--tone");
const PAGE = "http://localhost:8766/";
const DEBUG_PORT = 9336;
const SETTLE_MS = 3000;
const LISTEN_MS = 2000;
const POLL_MS = 250;
const POLL_TRIES = 60;
const TONE_KEY = "7";
// 20 ms pieces, so about 50 a second, of which this many must arrive.
const MIN_PIECES_PER_SECOND = 40;
// The test tone is about a fifth of full scale; the line's noise is far below this.
const MIN_TONE_PEAK = 0.1;
const MS_PER_SECOND = 1000;

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

// Wraps the browser's playback so each scheduled piece is counted and measured.
const HOOK = `
  window.pieces = [];
  const realStart = AudioBufferSourceNode.prototype.start;
  AudioBufferSourceNode.prototype.start = function (when, ...rest) {
    const peak = Math.max(...Array.from({ length: this.buffer.numberOfChannels }, (_, channel) =>
      this.buffer.getChannelData(channel).reduce((most, value) => Math.max(most, Math.abs(value)), 0)));
    window.pieces.push({ ahead: when - this.context.currentTime, peak, state: this.context.state });
    return realStart.call(this, when, ...rest);
  };
  0`;

const profile = mkdtempSync(join(tmpdir(), "kitsune-sound-"));
// Without --password-store=basic, Chromium waits on KDE Wallet and navigation hangs.
const browser = spawn("chromium", ["--headless=new", "--disable-gpu", "--no-first-run", "--password-store=basic", "--mute-audio",
  `--user-data-dir=${profile}`, `--remote-debugging-port=${DEBUG_PORT}`, "--remote-debugging-address=127.0.0.1", "about:blank"],
{ stdio: "ignore" });
const finish = (code) => {
  browser.kill();
  setTimeout(() => {
    rmSync(profile, { recursive: true, force: true });
    process.exit(code);
  }, POLL_MS);
};

const connect = async () => {
  for (let attempt = 0; attempt < POLL_TRIES; attempt++) {
    try {
      const targets = await (await fetch(`http://127.0.0.1:${DEBUG_PORT}/json`)).json();
      const target = targets.find((item) => item.type === "page");
      if (target) return target;
    } catch {
      // Not listening yet.
    }
    await sleep(POLL_MS);
  }
  throw new Error("the browser never became ready");
};

let failures = 0;
const check = (label, good, detail) => {
  if (!good) failures++;
  console.log(`${good ? "PASS" : "FAIL"}: ${label} ${detail}`);
};

try {
  const socket = new WebSocket((await connect()).webSocketDebuggerUrl);
  await new Promise((resolve, reject) => {
    socket.onopen = resolve;
    socket.onerror = reject;
  });
  let nextId = 1;
  const waiting = new Map();
  socket.onmessage = (message) => {
    const data = JSON.parse(message.data);
    if (waiting.has(data.id)) {
      waiting.get(data.id)(data);
      waiting.delete(data.id);
    }
  };
  const send = (method, params = {}) => new Promise((resolve) => {
    const id = nextId++;
    waiting.set(id, resolve);
    socket.send(JSON.stringify({ id, method, params }));
  });
  const evaluate = async (expression) => (await send("Runtime.evaluate", { expression, returnByValue: true })).result.result.value;
  const press = async (key) => {
    await send("Input.dispatchKeyEvent", { type: "keyDown", key, text: key });
    await send("Input.dispatchKeyEvent", { type: "keyUp", key });
  };

  await send("Page.navigate", { url: PAGE });
  await sleep(SETTLE_MS);
  await evaluate(HOOK);
  await press("s");
  await sleep(LISTEN_MS / 2);
  check("pressing s turns sound on", (await evaluate(`document.getElementById("sent").textContent`)) === "sound on",
    await evaluate(`document.getElementById("sent").textContent`));
  if (TONE) await fetch(new URL("/key", PAGE), { method: "POST", body: TONE_KEY });
  await sleep(LISTEN_MS);
  const pieces = await evaluate("window.pieces");
  const seconds = (LISTEN_MS * 1.5) / MS_PER_SECOND;
  check("pieces keep arriving", pieces.length >= MIN_PIECES_PER_SECOND * seconds, `${pieces.length} in about ${seconds} s`);
  check("the browser is playing", pieces.every((piece) => piece.state === "running"), `states: ${[...new Set(pieces.map((piece) => piece.state))]}`);
  const aheads = pieces.map((piece) => piece.ahead * MS_PER_SECOND);
  console.log(`queued ahead: ${Math.min(...aheads).toFixed(0)} to ${Math.max(...aheads).toFixed(0)} ms`);
  const loudest = Math.max(...pieces.map((piece) => piece.peak));
  if (TONE) check("the tone comes through", loudest >= MIN_TONE_PEAK, `loudest piece peaks at ${loudest.toFixed(3)}`);
  else console.log(`loudest piece peaks at ${loudest.toFixed(3)}`);

  await press("s");
  await sleep(POLL_MS);
  const before = (await evaluate("window.pieces")).length;
  await sleep(LISTEN_MS / 2);
  const after = (await evaluate("window.pieces")).length;
  check("pressing s again stops it", after === before && (await evaluate(`document.getElementById("sent").textContent`)) === "sound off",
    `${after - before} pieces after stopping`);

  if (TONE) {
    await press(TONE_KEY);
    await sleep(LISTEN_MS);
    const later = (await evaluate("window.pieces")).slice(after);
    const laterSeconds = LISTEN_MS / MS_PER_SECOND;
    check("7 on the page turns sound on", later.length >= MIN_PIECES_PER_SECOND * laterSeconds, `${later.length} pieces in ${laterSeconds} s`);
    const laterLoudest = Math.max(0, ...later.map((piece) => piece.peak));
    check("and its tone is heard", laterLoudest >= MIN_TONE_PEAK, `loudest piece peaks at ${laterLoudest.toFixed(3)}`);
    await press("s");
  }
  finish(failures ? 1 : 0);
} catch (error) {
  console.error(`sound_live: ${error.message}`);
  finish(1);
}
