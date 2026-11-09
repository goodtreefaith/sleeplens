import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  AppInfo, CardCandidate, DateRange, DragDropEvent, ImportProgress, ImportResult, ImportStarted,
  PeriodSummary, NightDetail, NightListItem, SettingsChange, SleepLensApi, Trends, Unlisten
} from "./types";

/** Drops undefined keys so optional args arrive as missing (Option::None) on the Rust side. */
function rangeArgs(range?: DateRange): Record<string, string> {
  const args: Record<string, string> = {};
  if (range?.from) args.from = range.from;
  if (range?.to) args.to = range.to;
  return args;
}

export function createTauriApi(): SleepLensApi {
  return {
    kind: "tauri",
    appInfo: () => invoke<AppInfo>("app_info"),
    findCards: () => invoke<CardCandidate[]>("find_cards"),
    importCard: (path) => invoke<ImportStarted>("import_card", { path }),
    cancelImport: () => invoke<null>("cancel_import"),
    listNights: (range) => invoke<NightListItem[]>("list_nights", rangeArgs(range)),
    nightDetail: (date) => invoke<NightDetail>("night_detail", { date }),
    trends: (range) => invoke<Trends>("trends", rangeArgs(range)),
    periodSummary: (period, end) => invoke<PeriodSummary>("period_summary", { period, ...(end ? { end } : {}) }),
    settingsHistory: () => invoke<SettingsChange[]>("settings_history"),
    deleteAllData: () => invoke<null>("delete_all_data", { confirm: true }),
    loadSampleData: () => invoke<ImportStarted>("load_sample_data"),

    onImportProgress: (handler) => listen<ImportProgress>("import-progress", (e) => handler(e.payload)),
    onImportFinished: (handler) => listen<ImportResult>("import-finished", (e) => handler(e.payload)),

    async pickFolder() {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const picked = await open({ directory: true, multiple: false, title: "Choose your SD card" });
      return typeof picked === "string" ? picked : null;
    },

    async onDragDrop(handler: (e: DragDropEvent) => void): Promise<Unlisten | null> {
      const { getCurrentWebview } = await import("@tauri-apps/api/webview");
      return getCurrentWebview().onDragDropEvent((event) => {
        const p = event.payload;
        if (p.type === "leave") handler({ type: "leave" });
        else if (p.type === "drop") handler({ type: "drop", paths: p.paths });
        else if (p.type === "enter") handler({ type: "enter", paths: p.paths });
        else handler({ type: "over", paths: [] });
      });
    }
  };
}
