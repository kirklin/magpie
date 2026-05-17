import { Clipboard, Search } from "lucide-react";

interface EmptyStateProps {
  icon?: "clipboard" | "search";
  title: string;
  subtitle?: string;
}

export function EmptyState({ icon = "clipboard", title, subtitle }: EmptyStateProps) {
  const isSearch = icon === "search";
  const IconComponent = isSearch ? Search : Clipboard;

  return (
    <div className="flex flex-col items-center justify-center gap-4 p-8 h-full text-center animate-fade-in select-none">
      {/* Layered icon with subtle glow */}
      <div className="relative">
        <div className="absolute inset-0 rounded-full bg-bg-hover blur-xl scale-150 opacity-60" />
        <div className="relative w-10 h-10 rounded-xl bg-bg-tertiary/60 border border-border flex items-center justify-center backdrop-blur-sm">
          <IconComponent size={18} className="text-text-tertiary" strokeWidth={1.5} />
        </div>
      </div>

      <div className="space-y-1.5">
        <div className="text-[13px] text-text-secondary/80 font-medium">{title}</div>
        {subtitle && (
          <div className="text-[11px] text-text-tertiary/60 max-w-48 leading-relaxed">{subtitle}</div>
        )}
      </div>
    </div>
  );
}
