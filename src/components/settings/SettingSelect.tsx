import { ChevronDown } from "lucide-react";

interface SettingSelectProps {
  label: string;
  value: string;
  options: { value: string; label: string }[];
  onChange: (value: string) => void;
}

export function SettingSelect({ label, value, options, onChange }: SettingSelectProps) {
  return (
    <div className="flex items-center justify-between px-4 py-3 min-h-[44px]">
      <span className="text-[13px] text-text-primary">{label}</span>
      <div className="relative flex items-center">
        <select
          value={value}
          onChange={e => onChange(e.target.value)}
          className="text-[13px] text-text-secondary bg-transparent border-none outline-none appearance-none pr-5 cursor-default focus:text-text-primary transition-colors text-right"
        >
          {options.map(opt => (
            <option key={opt.value} value={opt.value} className="bg-bg-secondary text-text-primary">
              {opt.label}
            </option>
          ))}
        </select>
        <ChevronDown className="absolute right-0 w-3.5 h-3.5 text-text-secondary pointer-events-none" />
      </div>
    </div>
  );
}
