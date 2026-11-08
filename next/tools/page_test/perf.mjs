// Measures the real video page: opens it in a throwaway headless browser and reports what
// its readout says, with the CPU used by that browser and by the server. It only watches,
// so nothing reaches the Pi, but it does need `cargo server` running.
// Usage: node tools/page_test/perf.mjs [seconds]
import { execSync, spawn } from "node:child_process";
import { mkdtempSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const SECONDS = Number(process.argv[2] ?? 10);
const PAGE = "http://localhost:8766/";
const DEBUG_PORT = 9335;
const WIDTH = 1920, HEIGHT = 1080;
/** Long enough for the page to load and settle before measuring. */
const SETTLE_MS = 4000;
const POLL_MS = 250;
const POLL_TRIES = 60;
const PERCENT = 100;
const TICKS_PER_SECOND = Number(execSync("getconf CLK_TCK"));
// A process's user and system CPU are these fields of /proc/PID/stat, after its name.
const UTIME_AFTER_NAME = 11, STIME_AFTER_NAME = 12;

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const ticksOf = (pid) => {
  try {
    const stat = readFileSync(`/proc/${pid}/stat`, "utf8");
    const fields = stat.slice(stat.lastIndexOf(")") + 2).split(" ");
    return Number(fields[UTIME_AFTER_NAME]) + Number(fields[STIME_AFTER_NAME]);
  } catch {
    return 0; // It has exited since being listed.
  }
};

const pidsWithArgument = (text) => readdirSync("/proc").filter((name) => /^\d+$/.test(name)).filter((pid) => {
  try {
    return readFileSync(`/proc/${pid}/cmdline`, "utf8").includes(text);
  } catch {
    return false;
  }
});

const profile = mkdtempSync(join(tmpdir(), "kitsune-perf-"));
// Without --password-store=basic, Chromium waits on KDE Wallet and navigation hangs.
const browser = spawn("chromium", ["--headless=new", "--disable-gpu", "--no-first-run", "--password-store=basic",
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

  await send("Emulation.setDeviceMetricsOverride", { width: WIDTH, height: HEIGHT, deviceScaleFactor: 1, mobile: false });
  await send("Page.navigate", { url: PAGE });
  await sleep(SETTLE_MS);

  const browserPids = pidsWithArgument(profile);
  const serverPids = execSync("pgrep -x server").toString().trim().split("\n");
  const total = (pids) => pids.reduce((sum, pid) => sum + ticksOf(pid), 0);
  const startBrowser = total(browserPids), startServer = total(serverPids), started = Date.now();
  const samples = [];
  for (let second = 0; second < SECONDS; second++) {
    await sleep(1000);
    const reply = await send("Runtime.evaluate", { expression: `document.getElementById("rate").textContent`, returnByValue: true });
    samples.push(reply.result.result.value);
  }
  const elapsed = (Date.now() - started) / 1000;
  const percentOf = (ticks) => Math.round(ticks / TICKS_PER_SECOND / elapsed * PERCENT);
  console.log(`last readouts: ${samples.slice(-3).join(" | ")}`);
  console.log(`browser ${percentOf(total(browserPids) - startBrowser)}% of a core, server ${percentOf(total(serverPids) - startServer)}%`);
  finish(0);
} catch (error) {
  console.error(`perf: ${error.message}`);
  finish(1);
}
