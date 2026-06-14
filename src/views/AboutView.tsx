import { openUrl } from "@tauri-apps/plugin-opener";
import { ArrowLeft } from "lucide-react";
import { useEffect, useState } from "react";
import { SettingGroup } from "../components/settings/SettingGroup";
import { SettingRow } from "../components/settings/SettingRow";
import { useT } from "../i18n";
import { useNavigationStore } from "../stores/navigation";

export function AboutView() {
  const { navigateTo } = useNavigationStore();
  const t = useT();
  const [version, setVersion] = useState("...");

  useEffect(() => {
    // Dynamically read version from Tauri config
    import("@tauri-apps/api/app").then(({ getVersion }) => {
      getVersion().then(v => setVersion(v));
    }).catch(() => setVersion("0.1.1"));
  }, []);
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
          {t("about.title")}
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto px-8 pb-10 flex flex-col items-center">
        <img src="/logo.png" alt="Magpie Logo" className="w-28 h-28 mb-3 mt-6 drop-shadow-2xl" />
        <h1 className="text-2xl font-bold text-text-primary mb-1">Magpie</h1>
        <p className="text-[11px] text-text-secondary mb-8">{t("about.tagline")}</p>

        <div className="w-full">
          <SettingGroup>
            <SettingRow label={t("about.name")} value="Magpie" />
            <SettingRow label={t("about.version")} value={`v${version}`} />
            <SettingRow
              label={t("about.developer")}
              value={(
                <button
                  className="transition-colors hover:text-text-primary"
                  onClick={() => openUrl("https://github.com/kirklin")}
                >
                  Kirk Lin
                </button>
              )}
            />
            <SettingRow
              label={t("about.repo")}
              value={(
                <button
                  className="transition-colors hover:text-text-primary"
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
