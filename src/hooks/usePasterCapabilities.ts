import type { PasterCapabilities } from "../bindings";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

// Fixed for the lifetime of the process, so fetched once and shared.
let cached: PasterCapabilities | null = null;
let pending: Promise<PasterCapabilities> | null = null;

function load(): Promise<PasterCapabilities> {
  pending ??= invoke<PasterCapabilities>("get_paster_capabilities").then((caps) => {
    cached = caps;
    return caps;
  });
  return pending;
}

/**
 * What paste-back can do on this desktop (e.g. Wayland can't bring another
 * app to the front). `null` until the first answer arrives.
 */
export function usePasterCapabilities(): PasterCapabilities | null {
  const [caps, setCaps] = useState(cached);

  useEffect(() => {
    if (cached) {
      return;
    }
    let cancelled = false;
    load().then((c) => {
      if (!cancelled) {
        setCaps(c);
      }
    });
    return () => {
      cancelled = true;
    };
  }, []);

  return caps;
}
