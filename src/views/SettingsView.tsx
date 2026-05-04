import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { ArrowLeft } from "lucide-react";
import { useEffect, useState } from "react";
import { SettingGroup } from "../components/settings/SettingGroup";
import { SettingRow } from "../components/settings/SettingRow";
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

          <SettingGroup title="存储">
            <SettingSelect
              label="历史保留时长"
              options={[
                { label: "7 天", value: "7" },
                { label: "30 天", value: "30" },
                { label: "3 个月", value: "90" },
                { label: "1 年", value: "365" },
                { label: "永久保留", value: "0" },
              ]}
              value={settings.history_retention_days.toString()}
              onChange={val => updateSetting("history_retention_days", Number.parseInt(val))}
            />
            <SettingSelect
              label="最大记录条数"
              options={[
                { label: "100 条", value: "100" },
                { label: "500 条", value: "500" },
                { label: "1000 条", value: "1000" },
                { label: "5000 条", value: "5000" },
              ]}
              value={settings.max_history_count.toString()}
              onChange={val => updateSetting("max_history_count", Number.parseInt(val))}
            />
          </SettingGroup>

          <SettingGroup title="快捷键">
            <SettingRow label="唤出剪贴板" value="Option + Space" />
          </SettingGroup>
        </div>
      </div>
    </div>
  );
}
