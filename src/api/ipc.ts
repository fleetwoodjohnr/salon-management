import { invoke } from "@tauri-apps/api/core";

/** Error shape produced by the Rust `AppError` serializer. */
export interface AppError {
  kind: "validation" | "not_found" | "conflict" | "no_workspace" | "provider" | "database" | "io" | "data" | "other";
  message: string;
  field: string | null;
}

export function isAppError(e: unknown): e is AppError {
  return typeof e === "object" && e !== null && "kind" in e && "message" in e;
}

export function errorMessage(e: unknown): string {
  if (isAppError(e)) return e.message;
  if (e instanceof Error) return e.message;
  return String(e);
}

/** Typed wrapper around Tauri `invoke`. Argument keys are camelCase on this side. */
export function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(cmd, args);
}
