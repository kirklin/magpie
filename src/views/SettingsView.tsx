import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { ArrowLeft, Download, Trash2, Upload } from "lucide-react";
import { useEffect, useState } from "react";
import { AccentColorPicker } from "../components/settings/AccentColorPicker";
import { SettingGroup } from "../components/settings/SettingGroup";
import { SettingSelect } from "../components/settings/SettingSelect";
import { SettingToggle } from "../components/settings/SettingToggle";
import { ShortcutRecorder } from "../components/settings/ShortcutRecorder";
import { ThemePicker } from "../components/settings/ThemePicker";
import { useToastStore } from "../components/Toast";
import { type Locale, useT } from "../i18n";
import { parseAppError } from "../lib/error";
import { useClipboardStore } from "../stores/clipboard";
import { useNavigationStore } from "../stores/navigation";
import { useSettingsStore } from "../stores/settings";

export function SettingsView() {
  const { navigateTo } = useNavigationStore();
  const { isLoading, loadSettings, settings, updateSetting } = useSettingsStore();
  const { clearHistory, exportHistory, importHistory } = useClipboardStore();
  const addToast = useToastStore(s => s.add);
  const t = useT();
  const [autostart, setAutostart] = useState(false);
  const [showClearConfirm, setShowClearConfirm] = useState(false);
  const [isExporting, setIsExporting] = useState(false);
  const [isImporting, setIsImporting] = useState(false);

  useEffect(() => {
    loadSettings();
    isEnabled().then(setAutostart).catch(console.error);

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        if (showClearConfirm) {
          setShowClearConfirm(false);
        } else {
          navigateTo("clipboard");
        }
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [loadSettings, navigateTo, showClearConfirm]);

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

  const handleClearHistory = async () => {
    try {
      await clearHistory();
    } catch (e) {
      addToast(parseAppError(e).message, "error");
    } finally {
      setShowClearConfirm(false);
    }
  };

  const handleExport = async () => {
    setIsExporting(true);
    try {
      const count = await exportHistory();
      if (count > 0) {
        addToast(t("settings.exported", { n: count }));
      }
    } catch (e) {
      addToast(t("settings.export_failed", { msg: parseAppError(e).message }), "error");
    } finally {
      setIsExporting(false);
    }
  };

  const handleImport = async () => {
    setIsImporting(true);
    try {
      const count = await importHistory();
      if (count > 0) {
        addToast(t("settings.imported", { n: count }));
      } else {
        addToast(t("settings.import_none"), "info");
      }
    } catch (e) {
      addToast(t("settings.import_failed", { msg: parseAppError(e).message }), "error");
    } finally {
      setIsImporting(false);
    }
  };

  const handleShortcutChange = async (shortcut: string) => {
    try {
      await updateSetting("global_shortcut", shortcut);
      addToast(t("settings.shortcut_updated"));
    } catch (e) {
      addToast(parseAppError(e).message, "error");
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
          {t("settings.title")}
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 px-5 pb-10 overflow-y-auto">
        <div className="mt-4">
          <SettingGroup title={t("settings.section.general")}>
            <SettingToggle
              checked={autostart}
              label={t("settings.autostart")}
              onChange={handleAutostartChange}
            />
            <SettingToggle
              checked={settings.show_menu_bar_icon}
              description={t("settings.autostart_desc")}
              label={t("settings.menubar_icon")}
              onChange={val => updateSetting("show_menu_bar_icon", val)}
            />
            <SettingSelect
              label={t("settings.default_action")}
              options={[
                { label: t("settings.action_paste"), value: "paste" },
                { label: t("settings.action_copy"), value: "copy" },
              ]}
              value={settings.default_action}
              onChange={val => updateSetting("default_action", val as "copy" | "paste")}
            />
            <SettingSelect
              label={t("settings.language")}
              options={[
                { label: "中文", value: "zh" },
                { label: "English", value: "en" },
              ]}
              value={settings.locale}
              onChange={val => updateSetting("locale", val as Locale)}
            />
          </SettingGroup>

          <SettingGroup title={t("settings.section.appearance")}>
            <ThemePicker
              value={settings.theme}
              onChange={val => updateSetting("theme", val)}
            />
            <AccentColorPicker
              value={settings.accent_color}
              onChange={val => updateSetting("accent_color", val)}
            />
          </SettingGroup>

          <SettingGroup title={t("settings.section.shortcut")}>
            <ShortcutRecorder
              label={t("settings.shortcut_toggle")}
              description={t("settings.shortcut_toggle_desc")}
              value={settings.global_shortcut}
              onChange={handleShortcutChange}
            />
          </SettingGroup>

          <SettingGroup title={t("settings.section.data")}>
            <div className="flex items-center justify-between px-4 py-3">
              <div>
                <div className="text-[13px] text-text-primary">{t("settings.export_history")}</div>
                <div className="text-[11px] text-text-tertiary mt-0.5">{t("settings.export_history_desc")}</div>
              </div>
              <button
                className="no-drag flex items-center gap-1.5 px-3 py-1.5 text-[12px] font-medium rounded-md transition-colors text-text-secondary bg-bg-hover hover:bg-bg-active hover:text-text-primary disabled:opacity-50"
                disabled={isExporting}
                onClick={handleExport}
              >
                <Download className="w-3.5 h-3.5" />
                {isExporting ? t("settings.exporting") : t("settings.export")}
              </button>
            </div>
            <div className="flex items-center justify-between px-4 py-3">
              <div>
                <div className="text-[13px] text-text-primary">{t("settings.import_history")}</div>
                <div className="text-[11px] text-text-tertiary mt-0.5">{t("settings.import_history_desc")}</div>
              </div>
              <button
                className="no-drag flex items-center gap-1.5 px-3 py-1.5 text-[12px] font-medium rounded-md transition-colors text-text-secondary bg-bg-hover hover:bg-bg-active hover:text-text-primary disabled:opacity-50"
                disabled={isImporting}
                onClick={handleImport}
              >
                <Upload className="w-3.5 h-3.5" />
                {isImporting ? t("settings.importing") : t("settings.import")}
              </button>
            </div>
            <div className="flex items-center justify-between px-4 py-3">
              <div>
                <div className="text-[13px] text-text-primary">{t("settings.clear_history")}</div>
                <div className="text-[11px] text-text-tertiary mt-0.5">{t("settings.clear_history_desc")}</div>
              </div>
              <button
                className="no-drag flex items-center gap-1.5 px-3 py-1.5 text-[12px] font-medium rounded-md transition-colors text-red-400 bg-red-500/10 hover:bg-red-500/20"
                onClick={() => setShowClearConfirm(true)}
              >
                <Trash2 className="w-3.5 h-3.5" />
                {t("settings.clear")}
              </button>
            </div>
          </SettingGroup>
        </div>
      </div>

      {/* Clear Confirmation Dialog */}
      {showClearConfirm && (
        <div
          className="absolute inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm"
          onClick={() => setShowClearConfirm(false)}
        >
          <div
            className="mx-6 w-full max-w-[300px] rounded-xl bg-bg-secondary border border-border-primary p-5 shadow-2xl"
            onClick={e => e.stopPropagation()}
          >
            <div className="text-[15px] font-semibold text-text-primary text-center">
              {t("settings.clear_confirm_title")}
            </div>
            <div className="mt-2 text-[13px] text-text-secondary text-center leading-relaxed">
              {t("settings.clear_confirm_desc")}
            </div>
            <div className="mt-5 flex gap-3">
              <button
                className="flex-1 py-2 text-[13px] font-medium rounded-lg bg-bg-hover text-text-primary hover:bg-bg-active transition-colors"
                onClick={() => setShowClearConfirm(false)}
              >
                {t("common.cancel")}
              </button>
              <button
                className="flex-1 py-2 text-[13px] font-medium rounded-lg bg-red-500 text-white hover:bg-red-600 transition-colors"
                onClick={handleClearHistory}
              >
                {t("settings.clear_confirm_title")}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
