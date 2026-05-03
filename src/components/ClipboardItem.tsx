import type { ClipboardEntry } from "../stores/clipboard";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Pin } from "lucide-react";
import { getTypeIcon } from "../utils/classifier";

interface ClipboardItemProps {
  entry: ClipboardEntry;
  isSelected: boolean;
  onClick: () => void;
  onDoubleClick: () => void;
}

export function ClipboardItem({ entry, isSelected, onClick, onDoubleClick }: ClipboardItemProps) {
  const displayName = entry.custom_name || entry.content_preview || entry.text_content || "(空)";
  const typeIcon = getTypeIcon(entry.content_type);
  const isImage = entry.content_type === "image" && entry.image_path;

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
        <Pin size={12} className="shrink-0 opacity-60" />
      )}
    </div>
  );
}
