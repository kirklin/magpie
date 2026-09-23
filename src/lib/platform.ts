// OS-dependent UI conventions: which modifier drives app shortcuts, how keys
// are labelled, and how file paths are split. Read from the webview's user
// agent so the answer is available synchronously on first render (WebKit on
// macOS reports "Macintosh", WebView2 "Windows", WebKitGTK "Linux").

export type Os = "macos" | "windows" | "linux";

function detectOs(): Os {
  const ua = typeof navigator === "undefined" ? "" : navigator.userAgent;
  if (ua.includes("Mac")) {
    return "macos";
  }
  if (ua.includes("Windows")) {
    return "windows";
  }
  return "linux";
}

export const OS: Os = detectOs();
export const IS_MAC = OS === "macos";

/** Key name (`KeyboardEvent.key`) of the modifier behind app shortcuts: ⌘ on macOS, Ctrl elsewhere. */
export const PRIMARY_MODIFIER_KEY = IS_MAC ? "Meta" : "Control";

/** Whether the shortcut modifier (⌘ on macOS, Ctrl elsewhere) is held. */
export function isPrimaryModifier(e: KeyboardEvent): boolean {
  return IS_MAC ? e.metaKey : e.ctrlKey;
}

/** Whether exactly the shortcut modifier is held, with no Alt/Shift/other modifier. */
export function isOnlyPrimaryModifier(e: KeyboardEvent): boolean {
  return isPrimaryModifier(e) && !e.altKey && !e.shiftKey && (IS_MAC ? !e.ctrlKey : !e.metaKey);
}

/** Key-cap labels, following each OS's own menus and docs. */
export const KEY_LABEL = IS_MAC
  ? { mod: "⌘", ctrl: "⌃", alt: "⌥", shift: "⇧", super: "⌘", enter: "↵", backspace: "⌫", delete: "⌦", tab: "⇥", escape: "⎋" }
  : { mod: "Ctrl", ctrl: "Ctrl", alt: "Alt", shift: "Shift", super: OS === "windows" ? "Win" : "Super", enter: "↵", backspace: "Backspace", delete: "Del", tab: "Tab", escape: "Esc" };

/** Last component of a file path (both separators are valid on Windows). */
export function basename(path: string): string {
  const parts = path.split(OS === "windows" ? /[\\/]/ : /\//);
  return parts.filter(Boolean).pop() ?? path;
}

/** Everything before the last component, including the trailing separator. */
export function dirname(path: string): string {
  return path.slice(0, path.length - basename(path).length);
}

/** File extension without the dot, or "" when the name has none. */
export function extension(path: string): string {
  const name = basename(path);
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1) : "";
}
