import type { ClipboardEntry } from "../stores/clipboard";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import Prism from "prismjs";
import React, { useEffect, useState } from "react";
import { getTypeLabel } from "../utils/classifier";
import "prismjs/themes/prism-tomorrow.css";

type FileCategory = "image" | "video" | "audio" | "pdf" | "text" | "other";

function getFileCategory(filePath: string): FileCategory {
  const ext = filePath.substring(filePath.lastIndexOf(".")).toLowerCase();
  if ([".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp", ".tiff", ".tif", ".ico", ".svg", ".heic", ".heif", ".avif"].includes(ext)) {
    return "image";
  }
  if ([".mp4", ".mov", ".webm", ".m4v", ".avi", ".mkv"].includes(ext)) {
    return "video";
  }
  if ([".mp3", ".wav", ".m4a", ".aac", ".ogg", ".flac", ".aiff"].includes(ext)) {
    return "audio";
  }
  if (ext === ".pdf") {
    return "pdf";
  }
  if ([".txt", ".md", ".json", ".js", ".ts", ".tsx", ".jsx", ".html", ".css", ".rs", ".py", ".go", ".c", ".cpp", ".h", ".sh", ".yaml", ".yml", ".xml", ".csv", ".log", ".ini", ".conf", ".toml"].includes(ext)) {
    return "text";
  }
  return "other";
}

function canPreview(filePath: string): boolean {
  return getFileCategory(filePath) !== "other";
}

function shortenPath(fullPath: string): string {
  const home = "/Users/";
  const idx = fullPath.indexOf(home);
  if (idx === 0) {
    const afterHome = fullPath.substring(home.length);
    const slashIdx = afterHome.indexOf("/");
    if (slashIdx !== -1) {
      return `~${afterHome.substring(slashIdx)}`;
    }
  }
  return fullPath;
}

function formatFileSize(bytes: number): string {
  if (bytes <= 0) {
    return "0 B";
  }
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
  if (bytes < 1024 * 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

/** Renders a preview for a single file based on its type */
function FilePreview({ filePath }: { filePath: string }) {
  const category = getFileCategory(filePath);
  const src = convertFileSrc(filePath);
  const [textContent, setTextContent] = useState<string | null>(null);

  useEffect(() => {
    if (category === "text") {
      let cancelled = false;
      fetch(src)
        .then(res => res.text())
        .then((text) => {
          if (!cancelled) {
            // Limit preview to ~10000 characters to avoid performance issues
            setTextContent(text.length > 10000 ? `${text.substring(0, 10000)}\n\n... (preview truncated)` : text);
          }
        })
        .catch(() => {
          if (!cancelled) {
            setTextContent("Failed to load text preview.");
          }
        });
      return () => {
        cancelled = true;
      };
    }
  }, [category, src]);

  switch (category) {
    case "image":
      return (
        <img
          src={src}
          alt=""
          className="max-w-full max-h-full object-contain rounded-lg shadow-md"
        />
      );
    case "video":
      return (
        <video
          src={src}
          controls
          className="max-w-full max-h-full rounded-lg shadow-md"
        />
      );
    case "audio":
      return (
        <div className="flex flex-col items-center gap-4">
          <svg width="64" height="64" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="text-text-tertiary">
            <path d="M9 18V5l12-2v13" />
            <circle cx="6" cy="18" r="3" />
            <circle cx="18" cy="16" r="3" />
          </svg>
          <audio src={src} controls className="w-full max-w-xs" />
        </div>
      );
    case "pdf":
      return (
        <iframe
          key={src}
          src={`${src}?t=${Date.now()}`}
          className="w-full h-full rounded-lg border-0 bg-white"
          title="PDF Preview"
        />
      );
    case "text":
      return (
        <div className="w-full h-full overflow-auto bg-bg-secondary rounded-lg p-3 border border-border">
          <pre className="text-[12px] text-text-primary font-sans whitespace-pre-wrap break-words leading-relaxed select-text cursor-text">
            {textContent === null ? "Loading..." : textContent}
          </pre>
        </div>
      );
    default:
      return null;
  }
}

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

// Async component to load native file icon from macOS
export function NativeFileIcon({ filePath, className = "w-16 h-16" }: { filePath: string; className?: string }) {
  const [iconSrc, setIconSrc] = useState<string | null>(null);

  useEffect(() => {
    if (!filePath) {
      return;
    }
    let cancelled = false;
    invoke<string>("get_file_icon", { filePath })
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
  }, [filePath]);

  if (iconSrc) {
    return <img src={iconSrc} alt="File icon" className={`object-contain ${className}`} />;
  }

  // Fallback: generic file icon
  return (
    <div className={`flex items-center justify-center bg-bg-tertiary rounded-lg text-text-tertiary ${className}`}>
      <svg width="50%" height="50%" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
        <path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z" />
        <path d="M14 2v4a2 2 0 0 0 2 2h4" />
      </svg>
    </div>
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
  const isFile = entry.content_type === "file" && entry.file_paths;
  const isColor = entry.content_type === "color";
  const isEmail = entry.content_type === "email";
  const isUrl = entry.content_type === "url";
  const isCode = entry.content_type === "code";
  const content = entry.text_content || "";

  // Parse file paths for file entries
  let filePaths: string[] = [];
  if (isFile) {
    try {
      filePaths = JSON.parse(entry.file_paths!);
    } catch {
      // fallback
    }
  }

  // Find the first previewable file
  const previewableFile = filePaths.find(canPreview) ?? null;

  const charCount = (isImage || isFile) ? 0 : content.length;
  const wordCount = (isImage || isFile) ? 0 : (content.trim() ? content.trim().split(/\s+/).length : 0);
  const lineCount = (isImage || isFile) ? 0 : content.split("\n").length;

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
          : isFile
            ? (
                <div className="flex flex-col h-full">
                  {/* File preview area */}
                  {previewableFile ? (
                    <div className="flex items-center justify-center flex-1 min-h-0">
                      <FilePreview filePath={previewableFile} />
                    </div>
                  ) : filePaths.length === 1 ? (
                  /* Single non-previewable file: show large native icon */
                    <div className="flex flex-col items-center justify-center flex-1 min-h-0 gap-4">
                      <NativeFileIcon filePath={filePaths[0]} className="w-32 h-32 drop-shadow-md" />
                      <div className="text-center px-4">
                        <div className="text-[16px] font-medium text-text-primary break-all">
                          {filePaths[0].split("/").pop() || filePaths[0]}
                        </div>
                        <div className="text-[12px] text-text-tertiary mt-1 uppercase tracking-widest font-semibold">
                          {filePaths[0].substring(filePaths[0].lastIndexOf(".") + 1) || "FILE"}
                        </div>
                      </div>
                    </div>
                  ) : (
                  /* Multiple files: show list with native icons */
                    <div className="space-y-2">
                      {filePaths.map((filePath, i) => {
                        const fileName = filePath.split("/").pop() || filePath;
                        const dirPath = filePath.substring(0, filePath.length - fileName.length);
                        return (
                          <div key={i} className="flex items-start gap-2.5 p-2.5 rounded-lg bg-bg-secondary">
                            <NativeFileIcon filePath={filePath} className="w-8 h-8 mt-0.5 shrink-0" />
                            <div className="min-w-0 flex-1 self-center">
                              <div className="text-[13px] text-text-primary font-medium truncate">{fileName}</div>
                              <div className="text-[11px] text-text-tertiary truncate mt-0.5">{shortenPath(dirPath)}</div>
                            </div>
                          </div>
                        );
                      })}
                    </div>
                  )}
                </div>
              )
            : isColor
              ? (
                  <div className="flex flex-col items-center justify-center h-full gap-5">
                    <div
                      className="w-32 h-32 rounded-2xl shadow-[inset_0_2px_4px_rgba(0,0,0,0.1),_0_8px_16px_rgba(0,0,0,0.1)] border border-border"
                      style={{ backgroundColor: content }}
                    />
                    <div className="text-[20px] font-mono font-medium text-text-primary uppercase tracking-wide">
                      {content}
                    </div>
                  </div>
                )
              : isEmail
                ? (() => {
                    const parts = content.split("@");
                    const username = parts[0];
                    const domain = parts.slice(1).join("@");
                    return (
                      <div className="flex flex-col items-center justify-center h-full px-12 text-center select-text cursor-text">
                        <div className="flex items-center justify-center flex-wrap text-[22px] tracking-tight leading-none break-all">
                          <span className="text-text-primary font-medium">{username}</span>
                          {domain && (
                            <>
                              <span className="text-text-tertiary font-light mx-[2px]">@</span>
                              <span className="text-text-secondary font-normal">{domain}</span>
                            </>
                          )}
                        </div>
                        <div className="mt-5 px-3 py-1 bg-bg-secondary/60 border border-border rounded-md text-[11px] font-medium text-text-tertiary tracking-widest uppercase shadow-sm">
                          Email Address
                        </div>
                      </div>
                    );
                  })()
                : isUrl
                  ? (() => {
                      let protocol = "";
                      let domain = content;
                      let path = "";

                      try {
                        const urlStr = content.startsWith("http") ? content : `https://${content}`;
                        const urlObj = new URL(urlStr);
                        protocol = `${urlObj.protocol}//`;
                        domain = urlObj.hostname;
                        path = urlObj.pathname + urlObj.search + urlObj.hash;
                        if (path === "/") {
                          path = "";
                        }
                      } catch {
                        // Fallback if parsing fails
                      }

                      return (
                        <div className="flex flex-col items-center justify-center h-full px-12 text-center select-text cursor-text">
                          <div className="flex items-center justify-center flex-wrap text-[20px] tracking-tight leading-relaxed max-w-full break-all">
                            {protocol && (
                              <span className="text-text-tertiary font-light mr-[1px]">{protocol}</span>
                            )}
                            <span className="text-text-primary font-medium">{domain}</span>
                            {path && (
                              <span className="text-text-secondary font-normal">{path}</span>
                            )}
                          </div>
                          <div className="mt-5 px-3 py-1 bg-bg-secondary/60 border border-border rounded-md text-[11px] font-medium text-text-tertiary tracking-widest uppercase shadow-sm">
                            Web Link
                          </div>
                        </div>
                      );
                    })()
                  : isCode
                    ? (() => {
                        let highlightedCode = content;
                        try {
                          highlightedCode = Prism.highlight(content, Prism.languages.javascript, "javascript");
                        } catch {
                          // Fallback to raw text
                        }
                        return (
                          <div className="h-full w-full relative flex flex-col">
                            <pre className="flex-1 overflow-auto bg-bg-secondary rounded-lg p-3 border border-border text-[13px] font-mono leading-[1.6] select-text cursor-text !m-0">
                              <code
                                className="language-javascript"
                                dangerouslySetInnerHTML={{ __html: highlightedCode }}
                              />
                            </pre>
                            <div className="absolute top-3 right-3 px-2 py-1 bg-black/40 backdrop-blur-md border border-white/10 rounded-md text-[10px] font-mono text-white/60 uppercase tracking-widest pointer-events-none shadow-sm">
                              Code
                            </div>
                          </div>
                        );
                      })()
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
          {isFile && filePaths.length === 1 && (
            <InfoRow label="Path" value={shortenPath(filePaths[0])} />
          )}
          {isFile && filePaths.length > 1 && (
            <InfoRow label="Files" value={filePaths.length.toString()} />
          )}
          {isFile && entry.byte_size > 0 && (
            <InfoRow label="File size" value={formatFileSize(entry.byte_size)} />
          )}
          {!isImage && !isFile && <InfoRow label="Characters" value={charCount.toLocaleString()} />}
          {wordCount > 0 && <InfoRow label="Words" value={wordCount.toLocaleString()} />}
          {lineCount > 1 && <InfoRow label="Lines" value={lineCount.toLocaleString()} />}
          {isImage && entry.content_preview && <InfoRow label="Size" value={entry.content_preview} />}
          {entry.access_count > 1
            ? (
                <>
                  <InfoRow label="Times copied" value={entry.access_count.toLocaleString()} />
                  <InfoRow label="Last copied" value={formatCopiedTime(entry.accessed_at)} />
                  <InfoRow label="First copied" value={formatCopiedTime(entry.created_at)} />
                </>
              )
            : (
                <InfoRow label="Copied" value={formatCopiedTime(entry.created_at)} />
              )}
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
      <span className="text-text-primary font-medium truncate ml-4 text-right">{value}</span>
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
