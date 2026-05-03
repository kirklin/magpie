import { Check, ChevronDown, Search } from "lucide-react";
import { useEffect, useRef, useState } from "react";

interface SearchBarProps {
  value: string;
  onChange: (value: string) => void;
  activeFilter: string | null;
  onFilterChange: (filter: string | null) => void;
  placeholder?: string;
}

const FILTER_OPTIONS = [
  { value: null, label: "All Types" },
  { value: "text", label: "Text Only" },
  { value: "image", label: "Images Only" },
  { value: "file", label: "Files Only" },
  { value: "url", label: "Links Only" },
  { value: "color", label: "Colors Only" },
];

export function SearchBar({ value, onChange, activeFilter, onFilterChange, placeholder = "Type to filter entries…" }: SearchBarProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [isDropdownOpen, setIsDropdownOpen] = useState(false);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const currentFilterLabel = FILTER_OPTIONS.find(o => o.value === activeFilter)?.label || "All Types";

  return (
    <div className="drag-region flex items-center h-12 px-4 gap-3 border-b border-border shrink-0 relative">
      <div className="no-drag w-5 h-5 flex items-center justify-center text-text-tertiary">
        <Search size={16} strokeWidth={2.5} />
      </div>
      <input
        ref={inputRef}
        className="no-drag flex-1 h-full border-none outline-none bg-transparent text-text-primary text-[15px] font-sans caret-accent placeholder:text-text-tertiary"
        type="text"
        value={value}
        onChange={e => onChange(e.target.value)}
        placeholder={placeholder}
        spellCheck={false}
        autoComplete="off"
      />
      {value && (
        <button
          className="no-drag w-5 h-5 flex items-center justify-center border-none bg-bg-tertiary text-text-secondary rounded-full cursor-pointer text-[10px] transition-all duration-100 hover:bg-bg-active hover:text-text-primary shrink-0"
          onClick={() => onChange("")}
          title="清除"
        >
          ✕
        </button>
      )}

      {/* Custom Dropdown */}
      <div className="relative no-drag">
        <button
          className="flex items-center gap-2 bg-transparent text-text-secondary text-[13px] px-2.5 py-1.5 rounded-md hover:bg-bg-tertiary transition-colors"
          onClick={() => setIsDropdownOpen(!isDropdownOpen)}
        >
          {currentFilterLabel}
          <ChevronDown size={14} className="opacity-70" />
        </button>

        {isDropdownOpen && (
          <>
            <div
              className="fixed inset-0 z-10"
              onClick={() => setIsDropdownOpen(false)}
            />
            <div className="absolute right-0 top-full mt-1 w-40 bg-bg-secondary border border-border rounded-lg shadow-xl py-1 z-20 animate-scale-in origin-top-right">
              {FILTER_OPTIONS.map(option => (
                <button
                  key={option.value || "all"}
                  className={`w-full flex items-center justify-between px-3 py-1.5 text-[13px] hover:bg-bg-active transition-colors ${
                    activeFilter === option.value ? "text-text-primary" : "text-text-secondary"
                  }`}
                  onClick={() => {
                    onFilterChange(option.value);
                    setIsDropdownOpen(false);
                  }}
                >
                  {option.label}
                  {activeFilter === option.value && <Check size={14} />}
                </button>
              ))}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
