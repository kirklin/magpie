import type { ClipboardEntry } from "../stores/clipboard";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Files, Pin } from "lucide-react";
import { useState } from "react";
import { useLocale, useT } from "../i18n";
import { getTypeIcon } from "../utils/classifier";
import { NativeFileIcon } from "./PreviewPanel";

interface ClipboardItemProps {
  entry: ClipboardEntry;
  isSelected: boolean;
  /** When set (1-9), show a floating quick-paste number badge */
  quickPasteIndex?: number;
  onClick: () => void;
  onDoubleClick: () => void;
}

export function ClipboardItem({ entry, isSelected, quickPasteIndex, onClick, onDoubleClick }: ClipboardItemProps) {
  const t = useT();
  const locale = useLocale();
  const typeIcon = getTypeIcon(entry.content_type);
  // Falls back to a non-image icon when the referenced file/image can't load
  // (e.g. the source file was deleted), instead of showing a broken thumbnail.
  const [thumbFailed, setThumbFailed] = useState(false);
  const isImage = entry.content_type === "image" && entry.image_path;
  const isFile = entry.content_type === "file" && entry.file_paths;
  const isColor = entry.content_type === "color";

  // Build display name based on content type
  let displayName = entry.custom_name || entry.content_preview || entry.text_content || t("item.empty");
  let fileImagePath: string | null = null;
  let parsedPaths: string[] = [];
  if (isFile && entry.file_paths) {
    try {
      parsedPaths = JSON.parse(entry.file_paths);
      if (!entry.custom_name) {
        if (parsedPaths.length === 1) {
          displayName = parsedPaths[0].split("/").pop() || parsedPaths[0];
        } else {
          // Show the first few file names, then cap with a count — otherwise
          // copying e.g. 100 files would build a huge label and hover title.
          const names = parsedPaths.map(p => p.split("/").pop() || p);
          const MAX_NAMES = 3;
          const sep = locale === "zh" ? "、" : ", ";
          displayName = names.length > MAX_NAMES
            ? t("item.files_summary", { names: names.slice(0, MAX_NAMES).join(sep), n: names.length })
            : names.join(sep);
        }
      }
      // Only real images get an inline <img> thumbnail. Videos are intentionally
      // excluded: an <img> with a video src makes the browser fetch the entire
      // (possibly huge) video file just to fail, which made scrolling janky.
      const imageExts = new Set([
        ".png",
        ".jpg",
        ".jpeg",
        ".gif",
        ".webp",
        ".bmp",
        ".tiff",
        ".heic",
        ".avif",
        ".svg",
      ]);
      const thumbPath = parsedPaths.find(p => imageExts.has(p.substring(p.lastIndexOf(".")).toLowerCase()));
      if (thumbPath) {
        fileImagePath = thumbPath;
      }
    } catch {
      // fallback to content_preview
    }
  }

  const isMultiFile = isFile && parsedPaths.length > 1;

  return (
    <div
      data-entry-id={entry.id}
      className={`relative flex items-center gap-2.5 px-3 py-2 rounded-md mx-2 my-0.5 select-none ${
        isSelected
          ? "bg-bg-active text-text-primary shadow-sm"
          : "text-text-primary hover:bg-bg-hover"
      }`}
      onClick={onClick}
      onDoubleClick={onDoubleClick}
      role="option"
      aria-selected={isSelected}
    >
      {(isImage && !thumbFailed)
        ? (
            <div className="w-6 h-6 rounded overflow-hidden shrink-0 bg-bg-tertiary">
              <img
                src={convertFileSrc(entry.image_path!)}
                alt=""
                className="w-full h-full object-cover"
                onError={() => setThumbFailed(true)}
              />
            </div>
          )
        : isMultiFile
          ? (
              <span className={`w-6 h-6 flex items-center justify-center shrink-0 ${
                isSelected ? "text-text-primary" : "text-text-tertiary"
              }`}
              >
                <Files size={18} strokeWidth={2} />
              </span>
            )
          : (isFile && fileImagePath && !thumbFailed)
              ? (
                  <div className="w-6 h-6 rounded overflow-hidden shrink-0 bg-bg-tertiary">
                    <img
                      src={convertFileSrc(fileImagePath)}
                      alt=""
                      className="w-full h-full object-cover"
                      onError={() => setThumbFailed(true)}
                    />
                  </div>
                )
              : (isFile && parsedPaths && parsedPaths.length === 1)
                  ? (
                      <div className="shrink-0 flex items-center justify-center">
                        <NativeFileIcon filePath={parsedPaths[0]} className="w-6 h-6" />
                      </div>
                    )
                  : isColor
                    ? (
                        <span className={`flex items-center justify-center shrink-0 w-6 h-6 ${
                          isSelected ? "text-text-primary" : "text-text-tertiary"
                        }`}
                        >
                          <div className="w-4 h-4 rounded-full border border-black/10 shadow-sm" style={{ backgroundColor: displayName }} />
                        </span>
                      )
                    : (
                        <span className={`flex items-center justify-center shrink-0 ${
                          isSelected ? "text-text-primary" : "text-text-tertiary"
                        }`}
                        >
                          {typeIcon}
                        </span>
                      )}

      <div className="flex-1 min-w-0">
        <div className="text-[13px] leading-tight truncate" title={displayName}>{displayName}</div>
      </div>

      {entry.is_pinned && (
        <Pin size={12} className="shrink-0 text-text-tertiary" />
      )}

      {/* Quick-paste number badge */}
      {quickPasteIndex != null && (
        <kbd className="quick-paste-badge">
          {quickPasteIndex}
        </kbd>
      )}
    </div>
  );
}
