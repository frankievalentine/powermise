import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { inTauri } from "./api";
import "./styles.css";

// Outside Tauri there is no native window vibrancy behind the page.
if (!inTauri) document.body.classList.add("browser");

// Embedded as the marketing-site demo, the host page can steer the app:
// `{ type: "powermise:navigate", route }` and `{ type: "powermise:theme", theme }`.
// Only same-origin messages are accepted.
if (!inTauri && window.parent !== window) {
  document.body.classList.add("embedded");
  window.addEventListener("message", (event) => {
    if (event.origin !== window.location.origin) return;
    const message = event.data as { type?: string; route?: unknown; theme?: string };
    if (message?.type === "powermise:navigate") {
      // A list of routes navigates through each, so detail pages keep the
      // right section highlighted (e.g. Discover, then a tool).
      const routes = Array.isArray(message.route) ? message.route : [message.route];
      for (const route of routes) {
        window.dispatchEvent(new CustomEvent("powermise:navigate", { detail: route }));
      }
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
