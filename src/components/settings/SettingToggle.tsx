interface SettingToggleProps {
  label: string;
  description?: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}

export function SettingToggle({ label, description, checked, onChange }: SettingToggleProps) {
  return (
    <div className="flex items-center justify-between px-4 py-3 min-h-[44px]">
      <div className="flex flex-col">
        <span className="text-[13px] text-text-primary">{label}</span>
        {description && <span className="text-[11px] text-text-secondary mt-0.5">{description}</span>}
      </div>
      <button
        type="button"
        className={`relative inline-flex h-5 w-8 flex-shrink-0 cursor-default rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none ${
          checked ? "bg-accent" : "bg-bg-tertiary"
        }`}
        onClick={() => onChange(!checked)}
      >
        <span
          className={`pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out ${
            checked ? "translate-x-3" : "translate-x-0"
          }`}
        />
      </button>
    </div>
  );
}
