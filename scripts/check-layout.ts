// Checks that no screen of the desktop app overflows sideways, at the
// window's minimum size, on every platform's layout — including the
// full-window first-run Welcome and the not-yet-activated shell steps (whose
// one-line setup command once widened the whole window).
//
//   bun run --cwd apps/desktop dev           # the browser demo, on port 1420
//   bun scripts/check-layout.ts              # exits 1 and lists any overflow
//
// Uses headless Chrome or Chromium (CHROME=<path>, or the usual install
// locations). CI runs it against a demo build (.github/workflows/ci.yml).
import { spawn } from "node:child_process";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const app = process.argv[2] ?? "http://localhost:1420/";
const browser =
  process.env.CHROME ??
  [
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/usr/bin/google-chrome",
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
  ].find((path) => existsSync(path));
if (!browser) throw new Error("No Chrome or Chromium found; set CHROME to its path.");

// The app's minimum window (tauri.conf.json) and its default size.
const sizes = [
  { width: 880, height: 560 },
  { width: 1180, height: 760 },
];
const platforms = ["darwin", "win32", "linux"];
const routes: unknown[] = [
  { kind: "home" },
  { kind: "welcome" },
  { kind: "installed" },
  { kind: "discover" },
  [{ kind: "installed" }, { kind: "tool", name: "node" }],
  { kind: "updates" },
  { kind: "activity" },
  { kind: "project", path: "/Users/demo/code/web-app" },
  [{ kind: "machine" }, { kind: "import" }],
  { kind: "health" },
  { kind: "machine" },
  { kind: "settings" },
  { kind: "harnesses" },
  { kind: "mcp" },
  { kind: "integrations" },
  { kind: "skills" },
  { kind: "setup", name: "engineering-agents" },
  { kind: "editor" },
];

const port = 9351;
const profile = mkdtempSync(join(tmpdir(), "powermise-layout-"));
const chrome = spawn(browser, [
  "--headless=new",
  `--remote-debugging-port=${port}`,
  `--user-data-dir=${profile}`,
  "--no-first-run",
  // GitHub's Ubuntu runners block the user namespaces Chrome's sandbox needs.
  ...(process.env.CI && process.platform === "linux" ? ["--no-sandbox"] : []),
  "about:blank",
]);
// Kept to explain a browser that never starts.
let chromeLog = "";
chrome.stderr?.on("data", (chunk) => (chromeLog = (chromeLog + chunk).slice(-4000)));
const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
let targets: { type: string; webSocketDebuggerUrl: string }[] = [];
for (let tries = 0; tries < 150 && !targets.length; tries++) {
  await sleep(200);
  try {
    targets = (await (await fetch(`http://127.0.0.1:${port}/json`)).json()).filter(
      (target: { type: string }) => target.type === "page",
    );
  } catch {}
}
if (!targets.length) {
  chrome.kill();
  console.error(`${browser} didn't start within 30 seconds.\n${chromeLog}`);
  process.exit(1);
}
const ws = new WebSocket(targets[0].webSocketDebuggerUrl);
await new Promise((resolve) => ws.addEventListener("open", resolve, { once: true }));
let id = 0;
const pending = new Map<number, (value: any) => void>();
ws.addEventListener("message", (event) => {
  const message = JSON.parse(String(event.data));
  if (message.id && pending.has(message.id)) {
    pending.get(message.id)!(message.result);
    pending.delete(message.id);
  }
});
const send = (method: string, params: object = {}) =>
  new Promise<any>((resolve) => {
    pending.set(++id, resolve);
    ws.send(JSON.stringify({ id, method, params }));
  });
const evaluate = async (expression: string) =>
  (await send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true })).result?.value;

// What overflows the window or the page's scroll area sideways.
const measure = `(() => {
  const out = [];
  const root = document.scrollingElement;
  if (root.scrollWidth > innerWidth + 1) out.push('the window scrolls sideways (' + root.scrollWidth + 'px in ' + innerWidth + 'px)');
  const content = document.querySelector('.content');
  if (content && content.scrollWidth > content.clientWidth + 1) {
    const edge = content.getBoundingClientRect().right;
    const culprit = [...content.querySelectorAll('*')]
      .filter((element) => element.getBoundingClientRect().right > edge + 1 && getComputedStyle(element).position !== 'fixed')
      .find((element) => ![...element.children].some((child) => child.getBoundingClientRect().right > edge + 1));
    out.push('the page scrolls sideways (' + content.scrollWidth + 'px in ' + content.clientWidth + 'px)' +
      (culprit ? ': ' + culprit.tagName.toLowerCase() + '.' + String(culprit.className).split(' ').join('.') : ''));
  }
  return out;
})()`;

const failures: string[] = [];
let checked = 0;
for (const platform of platforms) {
  for (const { width, height } of sizes) {
    for (const activated of [true, false]) {
      await send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile: false });
      // A fresh first run each time: Welcome fills the window until it's done.
      await send("Page.navigate", { url: "about:blank" });
      await evaluate("try { localStorage.clear() } catch {}");
      const url = new URL(app);
      url.searchParams.set("platform", platform);
      if (!activated) url.searchParams.set("notActivated", "");
      await send("Page.navigate", { url: url.toString() });
      await sleep(2500);
      const label = `${platform} ${width}×${height}${activated ? "" : " (shell not set up)"}`;
      const firstRun = await evaluate("document.querySelector('.window.onboarding') ? true : false");
      if (!firstRun) failures.push(`${label}: first launch didn't open the full-window Welcome`);
      for (const problem of (await evaluate(measure)) ?? []) failures.push(`${label}, first-run Welcome: ${problem}`);
      checked++;
      for (const route of routes) {
        for (const step of Array.isArray(route) ? route : [route]) {
          await evaluate(`window.dispatchEvent(new CustomEvent("powermise:navigate", { detail: ${JSON.stringify(step)} }))`);
          await sleep(120);
        }
        await sleep(700);
        const name = JSON.stringify(Array.isArray(route) ? route.at(-1) : route);
        for (const problem of (await evaluate(measure)) ?? []) failures.push(`${label}, ${name}: ${problem}`);
        checked++;
      }
    }
  }
}

ws.close();
chrome.kill();
rmSync(profile, { recursive: true, force: true });
if (failures.length) {
  console.error(`${failures.length} layout problem${failures.length === 1 ? "" : "s"} in ${checked} screens:`);
  for (const failure of failures) console.error(`  ${failure}`);
  process.exit(1);
}
console.log(`No layout overflow in ${checked} screens.`);
