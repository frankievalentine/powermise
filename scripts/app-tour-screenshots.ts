// Screenshots of the powermise browser demo for the app tour docs, taken with
// a headless Chrome or Chromium through the DevTools protocol.
//
//   bun run --cwd apps/desktop dev        # the demo, on port 1420
//   bun scripts/app-tour-screenshots.ts   # writes apps/web/src/assets/app-tour
//
// CHROME=<path> picks the browser; otherwise Chromium or Google Chrome from
// /Applications.
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const out = process.argv[2] ?? join(import.meta.dir, "../apps/web/src/assets/app-tour");
const app = process.argv[3] ?? "http://localhost:1420/?platform=darwin";
const browser =
  process.env.CHROME ??
  [
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  ].find((path) => existsSync(path));
if (!browser) throw new Error("No Chrome or Chromium found; set CHROME to its path.");
const port = 9339;
mkdirSync(out, { recursive: true });

// Upgrades a tool on the Updates page through the run sheet, so History has
// something in it.
const upgrade = (tool: string) => `(async () => {
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const row = [...document.querySelectorAll(".list-row")].find((row) => row.querySelector(".row-title")?.textContent.trim() === "${tool}");
  [...row.querySelectorAll("button")].find((b) => b.textContent.trim() === "Upgrade").click();
  await sleep(800);
  [...document.querySelectorAll(".sheet-foot button")].find((b) => b.classList.contains("primary")).click();
  for (let i = 0; i < 40 && ![...document.querySelectorAll(".sheet-foot button")].some((b) => b.textContent.trim() === "Done"); i++) await sleep(250);
  [...document.querySelectorAll(".sheet-foot button")].find((b) => b.textContent.trim() === "Done").click();
  await sleep(500);
})()`;

const shots: { file: string; route: unknown; wait?: number; before?: string[] }[] = [
  { file: "home", route: { kind: "home" } },
  { file: "welcome", route: { kind: "welcome" } },
  { file: "my-tools", route: { kind: "installed" } },
  { file: "discover", route: { kind: "discover" } },
  { file: "tool-page", route: [{ kind: "installed" }, { kind: "tool", name: "node" }] },
  { file: "updates", route: { kind: "updates" } },
  { file: "history", route: { kind: "activity" }, before: [upgrade("jq"), upgrade("terraform")] },
  { file: "projects", route: { kind: "project", path: "/Users/demo/code/web-app" } },
  { file: "import", route: [{ kind: "machine" }, { kind: "import" }] },
  { file: "health", route: { kind: "health" } },
  { file: "this-computer", route: { kind: "machine" } },
  { file: "settings", route: { kind: "settings" } },
  { file: "ai-agents", route: { kind: "harnesses" } },
  { file: "ai-setup", route: { kind: "setup", name: "engineering-agents" }, wait: 2500 },
];

const profile = mkdtempSync(join(tmpdir(), "powermise-screenshots-"));
const chrome = spawn(browser, [
  "--headless=new",
  `--remote-debugging-port=${port}`,
  `--user-data-dir=${profile}`,
  "--no-first-run",
  "--hide-scrollbars",
  "about:blank",
]);
const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

let targets: { type: string; webSocketDebuggerUrl: string }[] = [];
for (let tries = 0; tries < 50 && !targets.length; tries++) {
  await sleep(200);
  try {
    targets = (await (await fetch(`http://127.0.0.1:${port}/json`)).json()).filter((t: { type: string }) => t.type === "page");
  } catch {}
}
const ws = new WebSocket(targets[0].webSocketDebuggerUrl);
await new Promise((resolve) => ws.addEventListener("open", resolve, { once: true }));
let id = 0;
const pending = new Map<number, (value: any) => void>();
ws.addEventListener("message", (event) => {
  const message = JSON.parse(String(event.data));
  if (message.id && pending.has(message.id)) {
    pending.get(message.id)!(message.error ? Promise.reject(new Error(message.error.message)) : message.result);
    pending.delete(message.id);
  }
});
const send = (method: string, params: object = {}) =>
  new Promise<any>((resolve) => {
    pending.set(++id, resolve);
    ws.send(JSON.stringify({ id, method, params }));
  });
const evaluate = (expression: string) => send("Runtime.evaluate", { expression, awaitPromise: true });

await send("Emulation.setDeviceMetricsOverride", { width: 1200, height: 760, deviceScaleFactor: 2, mobile: false });
await send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: "light" }] });
await send("Page.navigate", { url: app });
await sleep(3000);
// The demo's "Preview as" switcher isn't part of the app.
await evaluate(`document.head.insertAdjacentHTML("beforeend", "<style>.platform-switcher{display:none!important}</style>")`);

for (const shot of shots) {
  for (const script of shot.before ?? []) {
    const result = await evaluate(script);
    if (result.exceptionDetails) console.error(shot.file, result.exceptionDetails.exception?.description);
  }
  const routes = Array.isArray(shot.route) ? shot.route : [shot.route];
  for (const route of routes) {
    await evaluate(`window.dispatchEvent(new CustomEvent("powermise:navigate", { detail: ${JSON.stringify(route)} }))`);
    await sleep(150);
  }
  await sleep(shot.wait ?? 1500);
  const { data } = await send("Page.captureScreenshot", { format: "png" });
  writeFileSync(join(out, `${shot.file}.png`), Buffer.from(data, "base64"));
  console.log(shot.file);
}
ws.close();
chrome.kill();
rmSync(profile, { recursive: true, force: true });
