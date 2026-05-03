import Database from "@tauri-apps/plugin-sql";
import { useEffect, useState } from "react";
import { ClipboardHistory } from "./views/ClipboardHistory";
import "./styles/global.css";

function App() {
  const [dbReady, setDbReady] = useState(false);

  useEffect(() => {
    // Initialize the SQLite database (this triggers migrations)
    Database.load("sqlite:magpie.db")
      .then(() => {
        console.log("Database loaded successfully");
        setDbReady(true);
      })
      .catch((err) => {
        console.error("Failed to load database:", err);
        // Still show the UI even if DB fails
        setDbReady(true);
      });
  }, []);

  return (
    <div className="h-full rounded-2xl border border-border shadow-window overflow-hidden backdrop-blur-[40px] backdrop-saturate-[180%] bg-bg-primary animate-scale-in">
      {dbReady
        ? (
            <ClipboardHistory />
          )
        : (
            <div className="flex items-center justify-center h-full text-text-secondary text-sm">
              初始化中…
            </div>
          )}
    </div>
  );
}

export default App;
