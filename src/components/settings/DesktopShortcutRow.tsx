import type { ShortcutBinding } from "../../bindings";
import { invoke } from "@tauri-apps/api/core";
import { Copy } from "lucide-react";
import { useT } from "../../i18n";
import { parseAppError } from "../../lib/error";
import { useToastStore } from "../../stores/toast";

type DesktopBinding = Exclude<ShortcutBinding, { kind: "native" }>;

interface DesktopShortcutRowProps {
  label: string;
  binding: DesktopBinding;
}

/**
 * The toggle shortcut where the desktop, not Magpie, owns it (Wayland): either
 * bound through the desktop's GlobalShortcuts portal, or to be set up by the
 * user in the system keyboard settings with Magpie's toggle command.
 */
export function DesktopShortcutRow({ label, binding }: DesktopShortcutRowProps) {
  const t = useT();
  const addToast = useToastStore(s => s.add);

  const configure = async () => {
    try {
      await invoke("configure_system_shortcut");
    } catch (e) {
      addToast(parseAppError(e).message, "error");
    }
  };

  if (binding.kind === "portal") {
    return (
      <div className="flex flex-col gap-2 px-4 py-3">
        <div className="flex items-center justify-between gap-4 min-h-[20px]">
          <div className="flex flex-col">
            <span className="text-[13px] text-text-primary">{label}</span>
            <span className="text-[11px] text-text-tertiary mt-0.5">{t("settings.shortcut_desktop_desc")}</span>
          </div>
          <div className="flex items-center gap-2 shrink-0">
            <span className="text-[12px] text-text-secondary">{binding.trigger ?? t("settings.shortcut_unassigned")}</span>
            {binding.can_configure && (
              <button
                type="button"
                className="no-drag px-3 py-1.5 text-[12px] font-medium rounded-md transition-colors text-text-secondary bg-bg-hover hover:bg-bg-active hover:text-text-primary"
                onClick={configure}
              >
                {t("settings.shortcut_change")}
              </button>
            )}
          </div>
        </div>
        {binding.trigger === null && (
          <>
            <span className="text-[11px] text-text-tertiary leading-relaxed">{t("settings.shortcut_portal_unassigned_desc")}</span>
            <ToggleCommand command={binding.command} />
          </>
        )}
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-2 px-4 py-3">
      <span className="text-[13px] text-text-primary">{label}</span>
      <span className="text-[11px] text-text-tertiary leading-relaxed">{t("settings.shortcut_manual_desc")}</span>
      <ToggleCommand command={binding.command} />
    </div>
  );
}

/** Magpie's toggle command, with a button that copies it. */
function ToggleCommand({ command }: { command: string }) {
  const t = useT();
  const addToast = useToastStore(s => s.add);

  const copyCommand = async () => {
    try {
      await invoke("copy_clipboard_entry", { text: command });
      addToast(t("settings.command_copied"));
    } catch (e) {
      addToast(parseAppError(e).message, "error");
    }
  };

  return (
    <div className="flex items-center gap-2">
      <code className="flex-1 min-w-0 truncate px-2 py-1.5 rounded-md bg-bg-hover text-[12px] font-mono text-text-primary select-text">
        {command}
      </code>
      <button
        type="button"
        className="no-drag flex items-center gap-1.5 px-3 py-1.5 text-[12px] font-medium rounded-md transition-colors text-text-secondary bg-bg-hover hover:bg-bg-active hover:text-text-primary shrink-0"
        onClick={copyCommand}
      >
        <Copy className="w-3.5 h-3.5" />
        {t("common.copy")}
      </button>
    </div>
  );
}
