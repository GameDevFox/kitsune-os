// Real reloads against a throwaway server: nothing here touches the Pi or the live server.
import { createServer } from "node:http";
import { readFileSync } from "node:fs";
const debugPort = process.argv[2];
const HTML = readFileSync(process.env.PAGE_HTML ?? new URL("../../server/src/video.html", import.meta.url), "utf8");
let version = "one", pageLoads = 0, versionStatus = 200;
const site = createServer((request, response) => {
  if (request.url === "/") { pageLoads++; response.setHeader("Content-Type", "text/html"); response.end(HTML); }
  else if (request.url === "/version") { response.statusCode = versionStatus; response.end(version); }
  else { response.statusCode = 503; response.end("no frames here"); }
});
await new Promise((resolve) => site.listen(0, "127.0.0.1", resolve));
const url = `http://127.0.0.1:${site.address().port}/`;
const targets = await (await fetch(`http://127.0.0.1:${debugPort}/json`)).json();
const socket = new WebSocket(targets.find((item) => item.type === "page").webSocketDebuggerUrl);
await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
let nextId = 1; const waiting = new Map();
socket.onmessage = (message) => { const data = JSON.parse(message.data); if (data.id && waiting.has(data.id)) { waiting.get(data.id)(data); waiting.delete(data.id); } };
const send = (method, params = {}) => new Promise((resolve, reject) => { const id = nextId++; const timer = setTimeout(() => reject(new Error(`no reply to ${method}`)), 10000); waiting.set(id, (reply) => { clearTimeout(timer); resolve(reply); }); socket.send(JSON.stringify({ id, method, params })); });
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const evaluate = async (expression) => (await send("Runtime.evaluate", { expression, returnByValue: true })).result.result?.value;
const press = async (name) => { await send("Input.dispatchKeyEvent", { type: "keyDown", key: name, text: name }); await send("Input.dispatchKeyEvent", { type: "keyUp", key: name }); await sleep(100); };
let failures = 0;
const check = (label, passed, detail = "") => { failures += passed ? 0 : 1; console.log(`${passed ? "PASS" : "*** FAIL"}: ${label} ${detail}`); };
const WAIT = 2500;
await send("Page.enable"); await send("Runtime.enable");
await send("Emulation.setDeviceMetricsOverride", { width: 1600, height: 900, deviceScaleFactor: 1, mobile: false });
await send("Page.navigate", { url });
await sleep(WAIT);
check("first load", pageLoads === 1, `${pageLoads}`);
await sleep(WAIT);
check("no reload while nothing changes", pageLoads === 1, `${pageLoads}`);

await press("+"); await press("+");
await evaluate("left = -300; top_ = -120; 0");
const before = await evaluate("[zoom, left, top_]");
version = "two"; await sleep(WAIT);
check("a new version reloads", pageLoads === 2, `${pageLoads}`);
const after = await evaluate("[zoom, left, top_]");
check("the view survives", JSON.stringify(before) === JSON.stringify(after), `${before} ${after}`);
await sleep(WAIT);
check("it reloads once, not repeatedly", pageLoads === 2, `${pageLoads}`);

await press(" ");
check("paused", await evaluate("paused"));
version = "three"; await sleep(WAIT);
check("waits while paused", pageLoads === 2, `${pageLoads}`);
await press(" "); await sleep(WAIT);
check("reloads after unpausing", pageLoads === 3, `${pageLoads}`);

await press("f"); await sleep(300);
const full = await evaluate("!!document.fullscreenElement");
check("fullscreen", full);
version = "four"; await sleep(WAIT);
check("waits while fullscreen", pageLoads === 3, `${pageLoads}`);
await press("f"); await sleep(WAIT);
check("reloads after leaving fullscreen", pageLoads === 4, `${pageLoads}`);

versionStatus = 500; version = "five"; await sleep(WAIT);
check("an error isn't a new version", pageLoads === 4, `${pageLoads}`);
const alive = await evaluate("!!document.getElementById('screen')");
versionStatus = 200; await sleep(WAIT);
check("recovers after an error", alive && pageLoads === 5, `${pageLoads}`);
console.log(failures ? `FAILED ${failures}` : "ALL PASS");
site.close();
process.exit(failures ? 1 : 0);
