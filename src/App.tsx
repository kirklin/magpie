import type { ViewName } from "./stores/navigation";
import { listen } from "@tauri-apps/api/event";
import Database from "@tauri-apps/plugin-sql";
import { useEffect, useState } from "react";
import { parseAppError } from "./lib/error";
import { useNavigationStore } from "./stores/navigation";
import { AboutView } from "./views/AboutView";
import { ClipboardHistory } from "./views/ClipboardHistory";
import { SettingsView } from "./views/SettingsView";
import "./styles/global.css";

function App() {
  const [dbReady, setDbReady] = useState(false);
  const [dbError, setDbError] = useState<string | null>(null);
  const { currentView, navigateTo } = useNavigationStore();

  useEffect(() => {
    // Initialize the SQLite database (this triggers migrations)
    Database.load("sqlite:magpie.db")
      .then(() => {
        setDbReady(true);
      })
      .catch((err) => {
        console.error("Failed to load database:", err);
        // Surface the failure instead of silently entering a broken UI.
        setDbError(parseAppError(err).message);
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
  } else if (dbError) {
    content = (
      <div className="flex flex-col items-center justify-center h-full gap-2 px-6 text-center">
        <div className="text-sm font-medium text-text-primary">数据库初始化失败</div>
        <div className="text-xs text-text-secondary break-all">{dbError}</div>
        <div className="text-xs text-text-secondary">请重启 Magpie；若反复出现，请检查磁盘空间或重新安装。</div>
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
