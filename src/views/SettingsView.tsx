import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { ArrowLeft } from "lucide-react";
import { useEffect, useState } from "react";
import { SettingGroup } from "../components/settings/SettingGroup";
import { SettingSelect } from "../components/settings/SettingSelect";
import { SettingToggle } from "../components/settings/SettingToggle";
import { useNavigationStore } from "../stores/navigation";
import { useSettingsStore } from "../stores/settings";

export function SettingsView() {
  const { navigateTo } = useNavigationStore();
  const { isLoading, loadSettings, settings, updateSetting } = useSettingsStore();
  const [autostart, setAutostart] = useState(false);

  useEffect(() => {
    loadSettings();
    isEnabled().then(setAutostart).catch(console.error);

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        navigateTo("clipboard");
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [loadSettings, navigateTo]);

  const handleAutostartChange = async (checked: boolean) => {
    try {
      if (checked) {
        await enable();
      } else {
        await disable();
      }
      setAutostart(checked);
    } catch (e) {
      console.error("Failed to toggle autostart", e);
    }
  };

  if (isLoading) {
    return null;
  }

  return (
    <div className="flex flex-col h-full bg-bg-primary">
      {/* Header */}
      <div className="drag-region flex shrink-0 items-center h-12 px-3">
        <button
          className="no-drag flex items-center justify-center w-6 h-6 transition-colors rounded-md text-text-secondary hover:bg-bg-hover hover:text-text-primary"
          onClick={() => navigateTo("clipboard")}
        >
          <ArrowLeft className="w-4 h-4" />
        </button>
        <div className="ml-2 text-sm font-medium text-text-primary">
          设置
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 px-5 pb-10 overflow-y-auto">
        <div className="mt-4">
          <SettingGroup title="通用">
            <SettingToggle
              checked={autostart}
              label="开机自启"
              onChange={handleAutostartChange}
            />
            <SettingSelect
              label="默认双击操作"
              options={[
                { label: "直接粘贴 (推荐)", value: "paste" },
                { label: "仅复制", value: "copy" },
              ]}
              value={settings.default_action}
              onChange={val => updateSetting("default_action", val as "copy" | "paste")}
            />
          </SettingGroup>


        </div>
      </div>
    </div>
  );
}
