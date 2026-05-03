import { Clipboard, Search } from "lucide-react";
import React from "react";

interface EmptyStateProps {
  icon?: "clipboard" | "search";
  title: string;
  subtitle?: string;
}

export function EmptyState({ icon = "clipboard", title, subtitle }: EmptyStateProps) {
  const IconComponent = icon === "search" ? Search : Clipboard;

  return (
    <div className="flex flex-col items-center justify-center gap-3 p-8 h-full text-center animate-fade-in">
      <div className="w-12 h-12 rounded-2xl bg-bg-tertiary flex items-center justify-center mb-1">
        <IconComponent size={24} className="text-text-tertiary" strokeWidth={1.5} />
      </div>
      <div className="text-[15px] text-text-secondary font-medium">{title}</div>
      {subtitle && (
        <div className="text-xs text-text-tertiary max-w-60 leading-relaxed">{subtitle}</div>
      )}
    </div>
  );
}
