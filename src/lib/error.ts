import type { AppError } from "../bindings";
import { getLocale, t } from "../i18n";

/** Localized fallback for AppError kinds that carry no `message`. */
function kindFallback(kind: string): string {
  if (kind === "DbUnavailable") {
    return t(getLocale(), "error.db_unavailable");
  }
  return kind;
}

/**
 * Normalize anything thrown by a Tauri command into a displayable
 * `{ kind, message }`.
 *
 * Commands now reject with a typed {@link AppError} (`{ kind, message }`); this
 * also tolerates plain strings and `Error` objects so callers can format any
 * caught value the same way (e.g. `toast.add(parseAppError(e).message, "error")`)
 * instead of interpolating the raw object (which renders as "[object Object]").
 */
export function parseAppError(error: unknown): { kind: string; message: string } {
  if (error && typeof error === "object" && "kind" in error) {
    const e = error as AppError;
    const message
      = "message" in e && typeof e.message === "string" ? e.message : kindFallback(e.kind);
    return { kind: e.kind, message };
  }
  if (error instanceof Error) {
    return { kind: "Other", message: error.message };
  }
  return { kind: "Other", message: typeof error === "string" ? error : String(error) };
}
