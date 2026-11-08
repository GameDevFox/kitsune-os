// Drives the video page in headless Chromium with real input events.
import { readFileSync, writeFileSync } from "node:fs";

const [debugPort, outDir] = process.argv.slice(2);
const HTML = readFileSync(process.env.PAGE_HTML ?? new URL("../../server/src/video.html", import.meta.url), "utf8");
// Test frames in place of the network, as headless Chromium can't navigate here.
const STUB = `
  window.frameNumber = 0;
  window.sends = [];
  window.mouseSends = [];
  window.sendStatus = 200;
  window.fetch = async (url, options) => {
    if (url === "/version") return new Response("fixed");
    if (url === "/mouse") {
      window.mouseSends.push(Array.from(new Int16Array(options.body)));
      return new Response("");
    }
    if (url === "/key") {
      window.sends.push({ method: options.method, body: options.body });
      return new Response("", { status: window.sendStatus });
    }
    if (url !== "/video") throw new Error("unexpected request for " + url);
    window.streamsOpened = (window.streamsOpened ?? 0) + 1;
    const part = async () => {
      const canvas = new OffscreenCanvas(1920, 1080);
      const context = canvas.getContext("2d");
      context.fillStyle = "#000";
      context.fillRect(0, 0, 1920, 1080);
      context.fillStyle = "#ff6000";
      context.fillRect(144, 630, 20, 20);
      context.fillRect(300 + (window.frameNumber++ % 400), 600, 40, 40);
      const image = new Uint8Array(await (await canvas.convertToBlob({ type: "image/png" })).arrayBuffer());
      const head = new TextEncoder().encode("--frame\\r\\nContent-Length: " + image.length + "\\r\\n\\r\\n");
      const whole = new Uint8Array(head.length + image.length + 2);
      whole.set(head);
      whole.set(image, head.length);
      whole.set([13, 10], head.length + image.length);
      return whole;
    };
    const signal = options?.signal;
    return new Response(new ReadableStream({
      start: (controller) => {
        signal?.addEventListener("abort", () => controller.error(new DOMException("stopped", "AbortError")));
      },
      pull: async (controller) => {
        do {
          await new Promise((resolve) => setTimeout(resolve, 30));
        } while (window.streamStopped && !signal?.aborted);
        if (!signal?.aborted) {
          controller.enqueue(await part());
        }
      },
    }));
  };
`;

const targets = await (await fetch(`http://127.0.0.1:${debugPort}/json`)).json();
const target = targets.find((item) => item.type === "page");
const socket = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((resolve, reject) => {
  socket.onopen = resolve;
  socket.onerror = reject;
});

let nextId = 1;
const waiting = new Map();
const errors = [];
socket.onmessage = (message) => {
  const data = JSON.parse(message.data);
  if (data.id && waiting.has(data.id)) {
    waiting.get(data.id)(data);
    waiting.delete(data.id);
  } else if (data.method === "Runtime.exceptionThrown") {
    errors.push(data.params.exceptionDetails.exception?.description ?? data.params.exceptionDetails.text);
  } else if (data.method === "Runtime.consoleAPICalled" && data.params.type === "error") {
    errors.push(JSON.stringify(data.params.args));
  }
};

const send = (method, params = {}) => new Promise((resolve, reject) => {
  const id = nextId++;
  const timer = setTimeout(() => reject(new Error(`no reply to ${method}`)), 10000);
  waiting.set(id, (reply) => { clearTimeout(timer); resolve(reply); });
  socket.send(JSON.stringify({ id, method, params }));
});
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const evaluate = async (expression) => {
  const reply = await send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
  if (reply.result.exceptionDetails) {
    throw new Error(JSON.stringify(reply.result.exceptionDetails));
  }
  return reply.result.result.value;
};
const text = (id) => evaluate(`document.getElementById(${JSON.stringify(id)}).textContent`);
const mouse = (type, x, y, extra = {}) => send("Input.dispatchMouseEvent", { type, x, y, ...extra });
const key = async (name, text_ = name) => {
  await send("Input.dispatchKeyEvent", { type: "keyDown", key: name, text: text_ });
  await send("Input.dispatchKeyEvent", { type: "keyUp", key: name });
};
const view = () => evaluate("({zoom, left, top_, paused})");
const shot = async (name) => {
  const reply = await send("Page.captureScreenshot", { format: "png" });
  writeFileSync(`${outDir}/${name}.png`, Buffer.from(reply.result.data, "base64"));
};

let failures = 0;
const check = (label, passed, detail = "") => {
  failures += passed ? 0 : 1;
  console.log(`${passed ? "PASS" : "*** FAIL"}: ${label} ${detail}`);
};

console.log("connected");
await send("Runtime.enable");
console.log("runtime on");
await send("Page.enable");
console.log("page on");
await send("Emulation.setDeviceMetricsOverride", { width: 1600, height: 900, deviceScaleFactor: 1, mobile: false });
await evaluate(`${STUB}; document.open(); document.write(${JSON.stringify(HTML)}); document.close();`);
console.log("page loaded");
await sleep(2500);

const sends = () => evaluate("window.sends");
const clear = () => evaluate("window.sends = []; 0");
const press = async (name, extra = {}) => {
  await send("Input.dispatchKeyEvent", { type: "keyDown", key: name, text: name, ...extra });
  await send("Input.dispatchKeyEvent", { type: "keyUp", key: name, ...extra });
  await sleep(50);
};
const CTRL = 2;
for (const name of ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"]) {
  await clear();
  await press(name);
  const got = await sends();
  check(`${name} sends itself`, got.length === 1 && got[0].body === name && got[0].method === "POST", JSON.stringify(got));
  check(`${name} shows it was sent`, (await text("sent")) === `sent ${name}`, await text("sent"));
}
for (const name of ["x", "y"]) {
  await clear();
  await press(name);
  check(`${name} sends nothing`, (await sends()).length === 0);
}
await clear();
await press("1", { modifiers: CTRL });
check("Ctrl+1 sends nothing", (await sends()).length === 0);
await clear();
await press("2", { autoRepeat: true });
check("a held key doesn't repeat", (await sends()).length === 0);

await evaluate("zoomCenter(4); 0");
const zoomed = (await view()).zoom;
await press("0");
await press("1");
check("0 and 1 leave the view alone", (await view()).zoom === zoomed, `${zoomed} ${(await view()).zoom}`);
await press("a");
check("a is 1:1", (await view()).zoom === 1, `${(await view()).zoom}`);
await evaluate("zoomCenter(3); 0");
await press("z");
const fitted = (await view()).zoom;
check("z fits", fitted < 1 && fitted > 0, `${fitted}`);

await evaluate("window.sendStatus = 500; 0");
await press("3");
check("a refused send says so", (await text("sent")).includes("couldn't send 3: 500"), await text("sent"));
await evaluate(`window.fetchOld = window.fetch; window.fetch = async (url, o) => { if (url === "/key") throw new Error("offline"); return window.fetchOld(url, o); }; 0`);
await press("3");
check("an unreachable server says so", (await text("sent")).includes("couldn't send 3: offline"), await text("sent"));
// Headless Chromium has no real pointer lock, so this stands in for one.
const move = (x, y) => evaluate(`document.dispatchEvent(new MouseEvent("mousemove", { movementX: ${x}, movementY: ${y} })); 0`);
await move(5, 5);
await sleep(100);
check("the mouse sends nothing until it's captured", (await evaluate("window.mouseSends.length")) === 0);
await evaluate(`Object.defineProperty(document, "pointerLockElement", { configurable: true, get: () => document.getElementById("screen") }); document.dispatchEvent(new Event("pointerlockchange")); 0`);
await move(3, -4);
await move(10, 20);
await sleep(100);
const moves = await evaluate("window.mouseSends");
const total = moves.reduce((sum, [x, y]) => [sum[0] + x, sum[1] + y], [0, 0]);
check("moves add up, and arrive as two numbers", moves.length >= 1 && total[0] === 13 && total[1] === 16, JSON.stringify(moves));
await evaluate("window.mouseSends = []; 0");
await move(40000, -40000);
await sleep(100);
const big = await evaluate("window.mouseSends");
const bigTotal = big.reduce((sum, [x, y]) => [sum[0] + x, sum[1] + y], [0, 0]);
check("a huge move splits rather than wrapping", big.every(([x, y]) => Math.abs(x) <= 32767 && Math.abs(y) <= 32767) && bigTotal[0] === 40000 && bigTotal[1] === -40000, JSON.stringify(big));
await evaluate(`Object.defineProperty(document, "pointerLockElement", { configurable: true, get: () => null }); document.dispatchEvent(new Event("pointerlockchange")); window.mouseSends = []; 0`);
await move(7, 7);
await sleep(100);
check("releasing stops it", (await evaluate("window.mouseSends.length")) === 0);
// The cyan pixel marker, the only thing drawn with strokeRect, stays away while the
// mouse is captured, as it would sit frozen where the pointer was locked. Frames come
// too slowly here to wait for, so draws are made, or caught, directly.
await evaluate(`window.markers = 0; const strokeRect = CanvasRenderingContext2D.prototype.strokeRect; CanvasRenderingContext2D.prototype.strokeRect = function (...args) { window.markers += 1; return strokeRect.apply(this, args); }; 0`);
await mouse("mouseMoved", 600, 300);
await sleep(50);
const markersDrawn = (action) => evaluate(`window.markers = 0; ${action}; window.markers`);
const lock = (on) => `Object.defineProperty(document, "pointerLockElement", { configurable: true, get: () => ${on ? 'document.getElementById("screen")' : "null"} }); document.dispatchEvent(new Event("pointerlockchange"))`;
check("the pixel marker shows on hover", (await markersDrawn("draw()")) === 1);
await evaluate(`${lock(true)}; 0`);
check("but not while the mouse is captured", (await markersDrawn("draw()")) === 0);
check("and it's back as soon as it's released", (await markersDrawn(lock(false))) === 1);
// The controls only appear when asked for.
const helpHidden = () => evaluate(`getComputedStyle(document.getElementById("help")).display === "none"`);
check("the controls start hidden", await helpHidden());

// A click on the video captures the mouse, and a drag pans it instead.
await evaluate(`window.captures = 0; document.getElementById("screen").requestPointerLock = () => { window.captures += 1; }; 0`);
const drag = async (fromX, toX) => {
  await mouse("mouseMoved", fromX, 300);
  await mouse("mousePressed", fromX, 300, { button: "left", buttons: 1, clickCount: 1 });
  await mouse("mouseMoved", toX, 300, { button: "left", buttons: 1 });
  await mouse("mouseReleased", toX, 300, { button: "left", clickCount: 1 });
  await sleep(50);
};
await drag(400, 400);
check("a click captures the mouse", (await evaluate("window.captures")) === 1);
await drag(400, 402);
check("a slightly shaky click still does", (await evaluate("window.captures")) === 2);
const beforePan = (await view()).left;
await drag(400, 500);
check("a drag doesn't", (await evaluate("window.captures")) === 2);
check("and it still pans", (await view()).left !== beforePan);
await press("m");
check("m captures it", (await evaluate("window.captures")) === 3);
await evaluate(`Object.defineProperty(document, "pointerLockElement", { configurable: true, get: () => document.getElementById("screen") }); 0`);
await drag(400, 400);
check("a click while captured does nothing more", (await evaluate("window.captures")) === 3);
await evaluate(`Object.defineProperty(document, "pointerLockElement", { configurable: true, get: () => null }); 0`);
await press("?");
await mouse("mousePressed", 800, 450, { button: "left", buttons: 1, clickCount: 1 });
await mouse("mouseReleased", 800, 450, { button: "left", clickCount: 1 });
await sleep(50);
check("a click on the controls doesn't", (await evaluate("window.captures")) === 3);
check("no capture button is left", await evaluate(`document.getElementById("capture") === null`));
await press("?");
await press("?");
check("? shows the controls", !(await helpHidden()));
await press("?");
check("? hides them again", await helpHidden());
await press("?");
// A real Esc has no text, unlike the keys `press` is for.
await send("Input.dispatchKeyEvent", { type: "rawKeyDown", key: "Escape", code: "Escape", windowsVirtualKeyCode: 27 });
await send("Input.dispatchKeyEvent", { type: "keyUp", key: "Escape", code: "Escape", windowsVirtualKeyCode: 27 });
await sleep(50);
check("Esc hides them", await helpHidden());
await clear();
await press("?");
check("? isn't sent to the Pi", (await sends()).length === 0);
await press("?");

// Good news fades, but a failure stays.
await evaluate("window.fetch = window.fetchOld; window.sendStatus = 200; 0");
await press("2");
check("a sent key shows", (await text("sent")) === "sent 2", await text("sent"));
await sleep(1700);
check("and then fades", (await text("sent")) === "", await text("sent"));
await evaluate("window.sendStatus = 500; 0");
await press("2");
await sleep(1700);
check("a failure stays", (await text("sent")).includes("couldn't send 2"), await text("sent"));
await evaluate("window.sendStatus = 200; 0");
// However a stream's bytes are divided up, the same frames come out.
const framesFrom = (divide) => evaluate(`(async () => {
  const encode = (text) => new TextEncoder().encode(text);
  const part = (text) => encode("--frame\\r\\nContent-Type: image/jpeg\\r\\nContent-Length: " + text.length + "\\r\\n\\r\\n" + text + "\\r\\n");
  const parts = [part("one"), part("frame two, \\r\\n\\r\\n with a gap"), part("")];
  const all = new Uint8Array(parts.reduce((sum, item) => [...sum, ...item], []));
  const chunks = (${divide})(all);
  const reader = { read: async () => (chunks.length ? { done: false, value: chunks.shift() } : { done: true }) };
  const blobs = [];
  await readFrames(reader, (blob) => blobs.push(blob));
  return JSON.stringify(await Promise.all(blobs.map((blob) => blob.text())));
})()`);
const EXPECTED_FRAMES = JSON.stringify(["one", "frame two, \r\n\r\n with a gap", ""]);
check("frames in one read all come out", (await framesFrom("(all) => [all]")) === EXPECTED_FRAMES);
const oneByOne = await framesFrom("(all) => Array.from(all, (byte) => new Uint8Array([byte]))");
check("frames in one-byte reads all come out", oneByOne === EXPECTED_FRAMES, oneByOne);
const bigFrame = await evaluate(`(async () => {
  const size = 3 * 1024 * 1024;
  const head = new TextEncoder().encode("--frame\\r\\nContent-Length: " + size + "\\r\\n\\r\\n");
  const chunks = [head, new Uint8Array(size).fill(7)];
  const reader = { read: async () => (chunks.length ? { done: false, value: chunks.shift() } : { done: true }) };
  const sizes = [];
  await readFrames(reader, (blob) => sizes.push(blob.size));
  return sizes[0];
})()`);
check("a frame bigger than the buffer still comes out", bigFrame === 3 * 1024 * 1024, bigFrame);

// One stream stays open, rather than a request per frame.
check("one stream carries all the frames", (await evaluate("window.streamsOpened")) === 1, await evaluate("window.streamsOpened"));
const framesBefore = await evaluate("window.frameNumber");
await sleep(300);
check("frames keep coming", (await evaluate("window.frameNumber")) > framesBefore);

// A browser that's behind decodes only the newest frame.
const decodes = await evaluate(`(async () => {
  const canvas = new OffscreenCanvas(4, 4);
  canvas.getContext("2d");
  const blob = await canvas.convertToBlob({ type: "image/png" });
  const original = window.createImageBitmap;
  let calls = 0;
  // The test stream is still running, so only this blob's decodes count.
  window.createImageBitmap = (...args) => { calls += args[0] === blob ? 1 : 0; return original(...args); };
  for (let index = 0; index < 8; index++) receiveFrame(blob);
  await new Promise((resolve) => setTimeout(resolve, 200));
  window.createImageBitmap = original;
  return calls;
})()`);
check("frames that arrive during a decode are skipped", decodes <= 2, decodes);
await sleep(100);

// Pausing stops the stream, and resuming starts another.
const opened = await evaluate("window.streamsOpened");
await press(" ");
await sleep(150);
const pausedAt = await evaluate("window.frameNumber");
await sleep(300);
check("pausing stops the frames", (await evaluate("window.frameNumber")) === pausedAt);
await press(" ");
await sleep(800);
check("resuming opens a new stream", (await evaluate("window.streamsOpened")) === opened + 1, await evaluate("window.streamsOpened"));
check("and frames come again", (await evaluate("window.frameNumber")) > pausedAt);

// The color readout only works with the overlay open, and reads the current frame.
await press("z");
const orangeAt = await evaluate(`JSON.stringify({ x: (154 * zoom + left) / (screen.width / innerWidth), y: (640 * zoom + top_) / (screen.height / innerHeight) })`).then(JSON.parse);
await press("?");
await mouse("mouseMoved", orangeAt.x, orangeAt.y);
await sleep(100);
check("the readout gives the color under the pointer", (await text("pixel")).includes("#ff6000"), await text("pixel"));
await press("?");

// A stream that goes quiet says so, and gets replaced.
const streamsBeforeSilence = await evaluate("window.streamsOpened");
await evaluate("window.streamStopped = true; 0");
await sleep(3700);
check("a silent stream is reported", (await text("status")).includes("no frame"), await text("status"));
await evaluate("window.streamStopped = false; 0");
await sleep(1500);
check("and it's replaced by a new one", (await evaluate("window.streamsOpened")) > streamsBeforeSilence, await evaluate("window.streamsOpened"));
check("which clears the report", (await text("status")) === "", await text("status"));

check("no page errors", errors.length === 0, errors.join(" "));
console.log(failures ? `FAILED ${failures}` : "ALL PASS");
process.exit(failures ? 1 : 0);
