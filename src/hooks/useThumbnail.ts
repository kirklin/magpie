import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

// Module-level cache of resolved thumbnails (original path -> asset URL to
// render). Mirrors the fileIconCache in PreviewPanel: a virtualized row
// re-mounts every time it scrolls back into view, and without this each remount
// would fire a fresh IPC round-trip.
const thumbCache = new Map<string, string>();

// In-flight requests, so N rows asking for the same image while scrolling
// collapse into ONE generation. Without this, a fast scroll through a fresh
// history could kick off dozens of concurrent PNG decodes.
const inFlight = new Map<string, Promise<string>>();

/** Bound both maps so a long session can't grow them without limit. */
const MAX_CACHE = 400;

function resolveThumb(imagePath: string): Promise<string> {
  const existing = inFlight.get(imagePath);
  if (existing) {
    return existing;
  }
  const promise = invoke<string>("get_thumbnail", { imagePath })
    .then((thumbPath) => {
      const url = convertFileSrc(thumbPath);
      if (thumbCache.size > MAX_CACHE) {
        thumbCache.clear();
      }
      thumbCache.set(imagePath, url);
      return url;
    })
    .finally(() => {
      inFlight.delete(imagePath);
    });
  inFlight.set(imagePath, promise);
  return promise;
}

/**
 * Resolve the image URL a list row should render for `imagePath`.
 *
 * Returns a downscaled thumbnail rather than the original capture. This is a
 * memory fix, not a cosmetic one: an `<img>` pointed at the original forces the
 * WebView to decode the entire full-size bitmap (~27 MB for a 3360x2158
 * screenshot) just to paint a 24pt row, and WebKit holds those decoded bitmaps
 * well past the row scrolling away.
 *
 * Returns `null` until resolved so callers can show a placeholder instead of
 * briefly painting the full-size original — rendering the original as a
 * "temporary" fallback would defeat the entire purpose.
 *
 * On failure it falls back to the original path: a correct-looking heavy
 * thumbnail beats a broken image.
 */
export function useThumbnail(imagePath: string | null | undefined): string | null {
  // Initialise from cache so a re-mounted row paints synchronously.
  const [src, setSrc] = useState<string | null>(() =>
    imagePath ? thumbCache.get(imagePath) ?? null : null,
  );

  useEffect(() => {
    if (!imagePath) {
      setSrc(null);
      return;
    }
    const cached = thumbCache.get(imagePath);
    if (cached) {
      setSrc(cached);
      return;
    }
    let cancelled = false;
    setSrc(null);
    resolveThumb(imagePath)
      .then((url) => {
        if (!cancelled) {
          setSrc(url);
        }
      })
      .catch(() => {
        // Generation failed (unreadable/undecodable source). Show the original
        // so the row isn't blank; this path is rare enough that its decode cost
        // doesn't undo the win.
        if (!cancelled) {
          setSrc(convertFileSrc(imagePath));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [imagePath]);

  return src;
}
