import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { inTauri } from "./api";
import { applyPlatform, detectPlatform, setPlatformFromMessage } from "./platform";
import "@fontsource-variable/geist";
import "@fontsource-variable/geist-mono";
import "./styles.css";

// Native materials (vibrancy, Mica) exist only inside Tauri.
document.body.classList.add(inTauri ? "native" : "browser");
applyPlatform(detectPlatform(inTauri));

// Embedded as the marketing-site demo, the host page can steer the app:
// `{ type: "powermise:navigate", route }`, `{ type: "powermise:theme", theme }`,
// and `{ type: "powermise:platform", platform }`.
// Only same-origin messages are accepted.
if (!inTauri && window.parent !== window) {
  document.body.classList.add("embedded");
  window.addEventListener("message", (event) => {
    if (event.origin !== window.location.origin) return;
    const message = event.data as { type?: string; route?: unknown; theme?: string; platform?: string };
    if (message?.type === "powermise:navigate") {
      // A list of routes navigates through each, so detail pages keep the
      // right section highlighted (e.g. Discover, then a tool).
      const routes = Array.isArray(message.route) ? message.route : [message.route];
      for (const route of routes) {
        window.dispatchEvent(new CustomEvent("powermise:navigate", { detail: route }));
      }
    } else if (message?.type === "powermise:platform") {
      setPlatformFromMessage(message.platform);
    } else if (message?.type === "powermise:theme") {
      document.documentElement.dataset.theme = message.theme === "light" ? "light" : "dark";
    }
  });
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
