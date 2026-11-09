import { createMockApi } from "./mock";
import { createTauriApi } from "./tauri";
import type { SleepLensApi } from "./types";

export * from "./types";

/** True inside the Tauri web view (Tauri 2 injects window.__TAURI_INTERNALS__). */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** Tauri IPC inside the app; realistic in-memory mock in a normal browser (`npm run dev`). */
export const api: SleepLensApi = isTauri() ? createTauriApi() : createMockApi({ persist: true });
