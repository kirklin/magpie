import { useCallback, useEffect, useRef, useState } from "react";

interface EditContentModalProps {
  isOpen: boolean;
  initialContent: string;
  onSave: (content: string) => void;
  onClose: () => void;
}

/**
 * Modal for editing clipboard entry text content.
 * Uses a full-height textarea with monospace font for code editing comfort.
 */
export function EditContentModal({ isOpen, initialContent, onSave, onClose }: EditContentModalProps) {
  const [content, setContent] = useState(initialContent);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Sync content when modal opens with new initialContent
  useEffect(() => {
    if (isOpen) {
      setContent(initialContent);
      requestAnimationFrame(() => {
        const el = textareaRef.current;
        if (el) {
          el.focus();
          // Place cursor at end
          el.selectionStart = el.selectionEnd = el.value.length;
        }
      });
    }
  }, [isOpen, initialContent]);

  const handleSave = useCallback(() => {
    onSave(content);
    onClose();
  }, [content, onSave, onClose]);

  // Handle keyboard shortcuts within the modal
  useEffect(() => {
    if (!isOpen) return;

    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      } else if (e.key === "s" && e.metaKey) {
        e.preventDefault();
        handleSave();
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [isOpen, onClose, handleSave]);

  if (!isOpen) return null;

  const hasChanged = content !== initialContent;

  return (
    <>
      {/* Backdrop */}
      <div
        className="fixed inset-0 z-40 bg-black/40 backdrop-blur-sm"
        onClick={onClose}
      />

      {/* Modal */}
      <div className="fixed inset-4 z-50 flex flex-col bg-bg-secondary/98 backdrop-blur-xl border border-border rounded-xl shadow-[0_8px_32px_rgba(0,0,0,0.3)] animate-scale-in overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between px-4 h-11 border-b border-border shrink-0">
          <span className="text-[13px] font-medium text-text-primary">Edit Content</span>
          <div className="flex items-center gap-2">
            <button
              className="px-3 py-1 text-[12px] text-text-secondary hover:text-text-primary transition-colors rounded-md hover:bg-bg-hover"
              onClick={onClose}
            >
              Cancel
              <kbd className="ml-1.5 text-[10px] text-text-tertiary">Esc</kbd>
            </button>
            <button
              className={`px-3 py-1 text-[12px] rounded-md transition-colors ${
                hasChanged
                  ? "bg-accent text-white hover:bg-accent-hover"
                  : "bg-bg-hover text-text-tertiary cursor-not-allowed"
              }`}
              onClick={handleSave}
              disabled={!hasChanged}
            >
              Save
              <kbd className="ml-1.5 text-[10px] opacity-60">⌘S</kbd>
            </button>
          </div>
        </div>

        {/* Editor */}
        <div className="flex-1 overflow-hidden p-2">
          <textarea
            ref={textareaRef}
            className="w-full h-full bg-transparent border-none outline-none text-text-primary text-[13px] leading-relaxed font-mono resize-none p-2"
            value={content}
            onChange={(e) => setContent(e.target.value)}
            spellCheck={false}
          />
        </div>

        {/* Footer info */}
        <div className="flex items-center justify-between px-4 h-8 border-t border-border shrink-0">
          <span className="text-[11px] text-text-tertiary">
            {content.length} characters · {content.split("\n").length} lines
          </span>
          {hasChanged && (
            <span className="text-[11px] text-orange-400/60">Modified</span>
          )}
        </div>
      </div>
    </>
  );
}
