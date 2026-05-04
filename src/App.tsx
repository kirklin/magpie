import type { ViewName } from "./stores/navigation";
import { listen } from "@tauri-apps/api/event";
import Database from "@tauri-apps/plugin-sql";
import { useEffect, useState } from "react";
import { useNavigationStore } from "./stores/navigation";
import { AboutView } from "./views/AboutView";
import { ClipboardHistory } from "./views/ClipboardHistory";
import { SettingsView } from "./views/SettingsView";
import "./styles/global.css";

function App() {
  const [dbReady, setDbReady] = useState(false);
  const { currentView, navigateTo } = useNavigationStore();

  useEffect(() => {
    // Initialize the SQLite database (this triggers migrations)
    Database.load("sqlite:magpie.db")
      .then(() => {
        setDbReady(true);
      })
      .catch((err) => {
        console.error("Failed to load database:", err);
        // Still show the UI even if DB fails
        setDbReady(true);
      });

    // Listen for navigation events from Rust backend (e.g. system tray)
    const unlisten = listen<string>("navigate", (event) => {
      const view = event.payload as ViewName;
      if (["clipboard", "settings", "about"].includes(view)) {
        navigateTo(view);
      }
    });

    return () => {
      unlisten.then(f => f());
    };
  }, [navigateTo]);

  let content;
  if (!dbReady) {
    content = (
      <div className="flex items-center justify-center h-full text-text-secondary text-sm">
        初始化中…
      </div>
    );
  } else {
    switch (currentView) {
      case "settings":
        content = <SettingsView />;
        break;
      case "about":
        content = <AboutView />;
        break;
      case "clipboard":
      default:
        content = <ClipboardHistory />;
        break;
    }
  }

  return (
    <div className="h-full rounded-2xl border border-border shadow-window overflow-hidden backdrop-blur-[40px] backdrop-saturate-[180%] bg-bg-primary animate-scale-in">
      {content}
    </div>
  );
}

export default App;
