import type { StringKey } from "../../i18n";
import type { AccentColorId } from "../../stores/settings";
import { Check } from "lucide-react";
import { useT } from "../../i18n";
import { ACCENT_PRESETS } from "../../stores/settings";

interface AccentColorPickerProps {
  value: AccentColorId;
  onChange: (color: AccentColorId) => void;
}

export function AccentColorPicker({ value, onChange }: AccentColorPickerProps) {
  const t = useT();
  return (
    <div className="flex items-center justify-between px-4 py-3 min-h-[44px]">
      <span className="text-[13px] text-text-primary">{t("settings.accent_label")}</span>
      <div className="flex items-center gap-2">
        {ACCENT_PRESETS.map((preset) => {
          const isActive = value === preset.id;
          return (
            <button
              key={preset.id}
              type="button"
              title={t(`color.${preset.id}` as StringKey)}
              className="no-drag relative w-[22px] h-[22px] rounded-full transition-transform duration-150 hover:scale-110 focus:outline-none"
              style={{ background: preset.swatch }}
              onClick={() => onChange(preset.id)}
            >
              {isActive && (
                <Check
                  size={12}
                  strokeWidth={3}
                  className="absolute inset-0 m-auto text-white drop-shadow-[0_1px_1px_rgba(0,0,0,0.3)]"
                />
              )}
            </button>
          );
        })}
      </div>
    </div>
  );
}
