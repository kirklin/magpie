import { openUrl } from "@tauri-apps/plugin-opener";
import { ArrowLeft } from "lucide-react";
import { useEffect } from "react";
import { SettingGroup } from "../components/settings/SettingGroup";
import { SettingRow } from "../components/settings/SettingRow";
import { useNavigationStore } from "../stores/navigation";

export function AboutView() {
  const { navigateTo } = useNavigationStore();

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        navigateTo("clipboard");
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [navigateTo]);

  return (
    <div className="flex flex-col h-full bg-bg-primary">
      {/* Header */}
      <div className="flex items-center h-12 drag-region px-3 shrink-0">
        <button
          onClick={() => navigateTo("clipboard")}
          className="no-drag w-6 h-6 flex items-center justify-center rounded-md text-text-secondary hover:bg-bg-hover hover:text-text-primary transition-colors"
        >
          <ArrowLeft className="w-4 h-4" />
        </button>
        <div className="ml-2 font-medium text-sm text-text-primary">
          关于 Magpie
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto px-8 pb-10 flex flex-col items-center">
        <img src="/logo.png" alt="Magpie Logo" className="w-28 h-28 mb-3 mt-6 drop-shadow-2xl" />
        <h1 className="text-2xl font-bold text-text-primary mb-1">Magpie</h1>
        <p className="text-[11px] text-text-secondary mb-8">极简的跨平台剪贴板管理器</p>

        <div className="w-full">
          <SettingGroup>
            <SettingRow label="名称" value="Magpie" />
            <SettingRow label="版本" value="v0.1.0" />
            <SettingRow
              label="开发者"
              value={(
                <button
                  className="transition-colors cursor-pointer hover:text-text-primary"
                  onClick={() => openUrl("https://github.com/kirklin")}
                >
                  Kirk Lin
                </button>
              )}
            />
            <SettingRow
              label="开源仓库"
              value={(
                <button
                  className="transition-colors cursor-pointer hover:text-text-primary"
                  onClick={() => openUrl("https://github.com/kirklin/magpie")}
                >
                  GitHub
                </button>
              )}
            />
          </SettingGroup>
        </div>
      </div>
    </div>
  );
}
