import type { ThemeMode } from "../../stores/settings";

interface ThemePickerProps {
  value: ThemeMode;
  onChange: (theme: ThemeMode) => void;
}

/** Miniature macOS-style window for theme preview */
function MiniWindow({ isDark }: { isDark: boolean }) {
  const bg = isDark ? "#1e1e2e" : "#ffffff";
  const titlebar = isDark ? "#333347" : "#f0f0f2";
  const sidebar = isDark ? "#252538" : "#f5f5f7";
  const line = isDark ? "rgba(255,255,255,0.08)" : "rgba(0,0,0,0.08)";
  const dotColors = ["#ff5f57", "#febc2e", "#28c840"];

  return (
    <div
      className="w-full h-full rounded-[3px] overflow-hidden flex flex-col"
      style={{ background: bg, border: `0.5px solid ${isDark ? "rgba(255,255,255,0.1)" : "rgba(0,0,0,0.1)"}` }}
    >
      <div className="flex items-center gap-[2px] px-[4px] py-[2.5px] shrink-0" style={{ background: titlebar }}>
        {dotColors.map(c => (
          <div key={c} className="w-[4px] h-[4px] rounded-full" style={{ background: c }} />
        ))}
      </div>
      <div className="flex flex-1 min-h-0">
        <div className="w-[28%] shrink-0 p-[3px] flex flex-col gap-[2px]" style={{ background: sidebar }}>
          <div className="h-[2.5px] rounded-full w-[80%]" style={{ background: line }} />
          <div className="h-[2.5px] rounded-full w-[55%]" style={{ background: line }} />
          <div className="h-[2.5px] rounded-full w-[70%]" style={{ background: line }} />
        </div>
        <div className="flex-1 p-[3px] flex flex-col gap-[2px]">
          <div className="h-[2.5px] rounded-full w-[85%]" style={{ background: line }} />
          <div className="h-[2.5px] rounded-full w-[60%]" style={{ background: line }} />
          <div className="h-[2.5px] rounded-full w-[75%]" style={{ background: line }} />
        </div>
      </div>
    </div>
  );
}

const THEMES: { key: ThemeMode; label: string }[] = [
  { key: "light", label: "浅色" },
  { key: "dark", label: "深色" },
  { key: "system", label: "自动" },
];

export function ThemePicker({ value, onChange }: ThemePickerProps) {
  return (
    <div className="flex items-center justify-between px-4 py-3 min-h-[44px]">
      <span className="text-[13px] text-text-primary">主题</span>
      <div className="flex items-start gap-3">
        {THEMES.map(({ key, label }) => {
          const isActive = value === key;
          return (
            <button
              key={key}
              type="button"
              className="no-drag flex flex-col items-center gap-1 group"
              onClick={() => onChange(key)}
            >
              <div
                className={`w-[76px] h-[50px] rounded-lg p-[3px] transition-all duration-150 ${
                  isActive
                    ? "ring-[1.5px] ring-accent"
                    : "ring-1 ring-white/10 group-hover:ring-white/20"
                }`}
                style={{
                  background: key === "light"
                    ? "linear-gradient(135deg, #a8c0f0, #d4a8e0, #f0c8a0)"
                    : key === "dark"
                      ? "linear-gradient(135deg, #1a1a3e, #2d1b4e, #1e2d4a)"
                      : "linear-gradient(90deg, #a8c0f0, #d4a8e0 40%, #1a1a3e 60%, #1e2d4a)",
                }}
              >
                {key === "system"
                  ? (
                      <div className="w-full h-full flex gap-[1px]">
                        <div className="flex-1 overflow-hidden rounded-l-[1px]">
                          <MiniWindow isDark={false} />
                        </div>
                        <div className="flex-1 overflow-hidden rounded-r-[1px]">
                          <MiniWindow isDark />
                        </div>
                      </div>
                    )
                  : <MiniWindow isDark={key === "dark"} />
                }
              </div>
              <span className={`text-[10px] transition-colors ${isActive ? "text-accent" : "text-text-tertiary"}`}>
                {label}
              </span>
            </button>
          );
        })}
      </div>
    </div>
  );
}
