import type { ClipboardEntry } from "../stores/clipboard";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import React, { useEffect, useState } from "react";
import { getTypeLabel } from "../utils/classifier";

// Async component to load native app icon from macOS
function NativeAppIcon({ bundleId, appName }: { bundleId: string; appName: string }) {
  const [iconSrc, setIconSrc] = useState<string | null>(null);

  useEffect(() => {
    if (!bundleId) {
      return;
    }
    let cancelled = false;
    invoke<string>("get_app_icon", { bundleId })
      .then((base64) => {
        if (!cancelled) {
          setIconSrc(base64);
        }
      })
      .catch(() => {
        // Fallback: no icon available
      });
    return () => {
      cancelled = true;
    };
  }, [bundleId]);

  if (iconSrc) {
    return <img src={iconSrc} alt={appName} className="w-4 h-4 rounded" />;
  }

  // Fallback: letter icon
  return (
    <span className="w-4 h-4 rounded bg-bg-tertiary flex items-center justify-center text-[10px] font-bold text-text-primary">
      {appName.charAt(0).toUpperCase()}
    </span>
  );
}

interface PreviewPanelProps {
  entry: ClipboardEntry | null;
}

export function PreviewPanel({ entry }: PreviewPanelProps) {
  if (!entry) {
    return (
      <div className="flex flex-col items-center justify-center h-full text-text-tertiary gap-3">
        <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="opacity-40">
          <rect x="3" y="3" width="18" height="18" rx="2" />
          <path d="M9 9h6v6H9z" />
        </svg>
        <span className="text-sm">选择一个条目查看详情</span>
      </div>
    );
  }

  const isImage = entry.content_type === "image" && entry.image_path;
  const content = entry.text_content || "";
  const charCount = isImage ? 0 : content.length;
  const wordCount = isImage ? 0 : (content.trim() ? content.trim().split(/\s+/).length : 0);
  const lineCount = isImage ? 0 : content.split("\n").length;

  return (
    <div className="flex flex-col h-full">
      {/* Content preview */}
      <div className="flex-1 overflow-auto p-4">
        {isImage
          ? (
              <div className="flex items-center justify-center h-full">
                <img
                  src={convertFileSrc(entry.image_path!)}
                  alt="Clipboard image"
                  className="max-w-full max-h-full object-contain rounded-lg shadow-md"
                />
              </div>
            )
          : (
              <pre className="text-[13px] text-text-primary font-sans whitespace-pre-wrap break-words leading-relaxed select-text cursor-text">
                {content || "(无文本内容)"}
              </pre>
            )}
      </div>

      {/* Information section */}
      <div className="border-t border-border shrink-0">
        <div className="px-4 py-2 text-[11px] text-text-tertiary font-semibold uppercase tracking-wider">
          Information
        </div>
        <div className="px-4 pb-3 space-y-1">
          {entry.source_app && entry.source_app_name && (
            <InfoRow
              label="Source"
              value={(
                <span className="flex items-center gap-1.5">
                  <NativeAppIcon bundleId={entry.source_app} appName={entry.source_app_name} />
                  {entry.source_app_name}
                </span>
              )}
            />
          )}
          <InfoRow label="Content type" value={getTypeLabel(entry.content_type)} />
          {!isImage && <InfoRow label="Characters" value={charCount.toLocaleString()} />}
          {wordCount > 0 && <InfoRow label="Words" value={wordCount.toLocaleString()} />}
          {lineCount > 1 && <InfoRow label="Lines" value={lineCount.toLocaleString()} />}
          {isImage && entry.content_preview && <InfoRow label="Size" value={entry.content_preview} />}
          <InfoRow label="Copied" value={formatCopiedTime(entry.created_at)} />
          {entry.is_pinned && <InfoRow label="Status" value="📌 Pinned" />}
        </div>
      </div>
    </div>
  );
}

function InfoRow({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between text-[13px] py-0.5">
      <span className="text-text-secondary">{label}</span>
      <span className="text-text-primary font-medium">{value}</span>
    </div>
  );
}

function formatCopiedTime(dateStr: string): string {
  const date = new Date(`${dateStr}Z`);
  const now = new Date();
  const isToday = date.toDateString() === now.toDateString();

  const time = date.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", second: "2-digit" });

  if (isToday) {
    return `Today at ${time}`;
  }

  const yesterday = new Date(now);
  yesterday.setDate(yesterday.getDate() - 1);
  if (date.toDateString() === yesterday.toDateString()) {
    return `Yesterday at ${time}`;
  }

  return `${date.toLocaleDateString("zh-CN")} ${time}`;
}
