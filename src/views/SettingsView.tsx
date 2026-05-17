import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { ArrowLeft, Download, Trash2, Upload } from "lucide-react";
import { useEffect, useState } from "react";
import { SettingGroup } from "../components/settings/SettingGroup";
import { SettingSelect } from "../components/settings/SettingSelect";
import { SettingToggle } from "../components/settings/SettingToggle";
import { useToastStore } from "../components/Toast";
import { useClipboardStore } from "../stores/clipboard";
import { useNavigationStore } from "../stores/navigation";
import { useSettingsStore } from "../stores/settings";

export function SettingsView() {
  const { navigateTo } = useNavigationStore();
  const { isLoading, loadSettings, settings, updateSetting } = useSettingsStore();
  const { clearHistory, exportHistory, importHistory } = useClipboardStore();
  const addToast = useToastStore(s => s.add);
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
    await clearHistory();
    setShowClearConfirm(false);
  };

  const handleExport = async () => {
    setIsExporting(true);
    try {
      const count = await exportHistory();
      if (count > 0) {
        addToast(`已导出 ${count} 条记录`);
      }
    } catch (e) {
      addToast(`导出失败: ${e}`, "error");
    } finally {
      setIsExporting(false);
    }
  };

  const handleImport = async () => {
    setIsImporting(true);
    try {
      const count = await importHistory();
      if (count > 0) {
        addToast(`已导入 ${count} 条新记录`);
      } else {
        addToast("没有新记录需要导入", "info");
      }
    } catch (e) {
      addToast(`导入失败: ${e}`, "error");
    } finally {
      setIsImporting(false);
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

          <SettingGroup title="数据">
            <div className="flex items-center justify-between px-4 py-3">
              <div>
                <div className="text-[13px] text-text-primary">导出历史记录</div>
                <div className="text-[11px] text-text-tertiary mt-0.5">导出所有记录为 JSON 文件</div>
              </div>
              <button
                className="no-drag flex items-center gap-1.5 px-3 py-1.5 text-[12px] font-medium rounded-md transition-colors text-accent bg-accent-muted hover:bg-accent/20 disabled:opacity-50"
                disabled={isExporting}
                onClick={handleExport}
              >
                <Download className="w-3.5 h-3.5" />
                {isExporting ? "导出中…" : "导出"}
              </button>
            </div>
            <div className="flex items-center justify-between px-4 py-3">
              <div>
                <div className="text-[13px] text-text-primary">导入历史记录</div>
                <div className="text-[11px] text-text-tertiary mt-0.5">从 JSON 文件导入，自动跳过重复</div>
              </div>
              <button
                className="no-drag flex items-center gap-1.5 px-3 py-1.5 text-[12px] font-medium rounded-md transition-colors text-accent bg-accent-muted hover:bg-accent/20 disabled:opacity-50"
                disabled={isImporting}
                onClick={handleImport}
              >
                <Upload className="w-3.5 h-3.5" />
                {isImporting ? "导入中…" : "导入"}
              </button>
            </div>
            <div className="flex items-center justify-between px-4 py-3">
              <div>
                <div className="text-[13px] text-text-primary">清空剪贴板历史</div>
                <div className="text-[11px] text-text-tertiary mt-0.5">已置顶的记录不会被删除</div>
              </div>
              <button
                className="no-drag flex items-center gap-1.5 px-3 py-1.5 text-[12px] font-medium rounded-md transition-colors text-red-400 bg-red-500/10 hover:bg-red-500/20"
                onClick={() => setShowClearConfirm(true)}
              >
                <Trash2 className="w-3.5 h-3.5" />
                清空
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
              确认清空
            </div>
            <div className="mt-2 text-[13px] text-text-secondary text-center leading-relaxed">
              将删除所有未置顶的剪贴板记录，此操作无法撤销。
            </div>
            <div className="mt-5 flex gap-3">
              <button
                className="flex-1 py-2 text-[13px] font-medium rounded-lg bg-bg-hover text-text-primary hover:bg-bg-active transition-colors"
                onClick={() => setShowClearConfirm(false)}
              >
                取消
              </button>
              <button
                className="flex-1 py-2 text-[13px] font-medium rounded-lg bg-red-500 text-white hover:bg-red-600 transition-colors"
                onClick={handleClearHistory}
              >
                确认清空
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
