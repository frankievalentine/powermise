import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { inTauri } from "./api";
import "./styles.css";

// Outside Tauri there is no native window vibrancy behind the page.
if (!inTauri) document.body.classList.add("browser");

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
