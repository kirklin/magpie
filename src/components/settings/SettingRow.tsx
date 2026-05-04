import React from "react";

interface SettingRowProps {
  label: string;
  value?: React.ReactNode;
}

export function SettingRow({ label, value }: SettingRowProps) {
  return (
    <div className="flex items-center justify-between px-4 py-3 min-h-[44px]">
      <span className="text-[13px] text-text-primary">{label}</span>
      {value && <span className="text-[13px] text-text-secondary">{value}</span>}
    </div>
  );
}
