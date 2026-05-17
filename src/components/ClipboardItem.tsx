import type { ClipboardEntry } from "../stores/clipboard";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Pin } from "lucide-react";
import { getTypeIcon } from "../utils/classifier";
import { NativeFileIcon } from "./PreviewPanel";

interface ClipboardItemProps {
  entry: ClipboardEntry;
  isSelected: boolean;
  onClick: () => void;
  onDoubleClick: () => void;
}

export function ClipboardItem({ entry, isSelected, onClick, onDoubleClick }: ClipboardItemProps) {
  const typeIcon = getTypeIcon(entry.content_type);
  const isImage = entry.content_type === "image" && entry.image_path;
  const isFile = entry.content_type === "file" && entry.file_paths;
  const isColor = entry.content_type === "color";

  // Build display name based on content type
  let displayName = entry.custom_name || entry.content_preview || entry.text_content || "(空)";
  let fileImagePath: string | null = null;
  let parsedPaths: string[] = [];
  if (isFile && entry.file_paths) {
    try {
      parsedPaths = JSON.parse(entry.file_paths);
      if (!entry.custom_name) {
        if (parsedPaths.length === 1) {
          displayName = parsedPaths[0].split("/").pop() || parsedPaths[0];
        } else {
          displayName = `${parsedPaths.length} 个文件`;
        }
      }
      // Check if any file can show a visual thumbnail (images & videos)
      const visualExts = new Set([
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
        ".mp4",
        ".mov",
        ".webm",
        ".m4v",
      ]);
      const thumbPath = parsedPaths.find(p => visualExts.has(p.substring(p.lastIndexOf(".")).toLowerCase()));
      if (thumbPath) {
        fileImagePath = thumbPath;
      }
    } catch {
      // fallback to content_preview
    }
  }

  return (
    <div
      className={`flex items-center gap-2.5 px-3 py-2 cursor-pointer rounded-md mx-2 my-0.5 select-none ${
        isSelected
          ? "bg-bg-active text-text-primary shadow-sm"
          : "text-text-primary hover:bg-bg-hover"
      }`}
      onClick={onClick}
      onDoubleClick={onDoubleClick}
      role="option"
      aria-selected={isSelected}
    >
      {isImage
        ? (
            <div className="w-6 h-6 rounded overflow-hidden shrink-0 bg-bg-tertiary">
              <img
                src={convertFileSrc(entry.image_path!)}
                alt=""
                className="w-full h-full object-cover"
              />
            </div>
          )
        : (isFile && fileImagePath)
            ? (
                <div className="w-6 h-6 rounded overflow-hidden shrink-0 bg-bg-tertiary">
                  <img
                    src={convertFileSrc(fileImagePath)}
                    alt=""
                    className="w-full h-full object-cover"
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
        <div className="text-[13px] leading-tight truncate">{displayName}</div>
      </div>

      {entry.is_pinned && (
        <Pin size={12} className="shrink-0 text-text-tertiary" />
      )}
    </div>
  );
}
