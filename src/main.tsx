import { getCurrentWindow } from "@tauri-apps/api/window";
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";

// Global drag handler for custom titlebar regions
// CSS -webkit-app-region:drag is unreliable on macOS transparent windows
document.addEventListener("mousedown", (e) => {
  const target = e.target as HTMLElement;

  // Check if click is on a no-drag element
  if (target.closest(".no-drag")) {
    return;
  }

  // Check if click is inside a drag-region
  if (target.closest(".drag-region")) {
    e.preventDefault();
    getCurrentWindow().startDragging();
  }
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
