import React from "react";

interface SettingGroupProps {
  title?: string;
  children: React.ReactNode;
}

export function SettingGroup({ title, children }: SettingGroupProps) {
  return (
    <div className="mb-6">
      {title && <div className="px-2 pb-2 text-[12px] font-medium text-text-secondary">{title}</div>}
      <div className="bg-bg-secondary border border-border rounded-xl overflow-hidden divide-y divide-border/50">
        {children}
      </div>
    </div>
  );
}
