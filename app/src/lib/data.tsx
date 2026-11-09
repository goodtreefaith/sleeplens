import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { api } from "../api";
import type { AppInfo, ImportProgress, ImportResult, NightListItem, SettingsChange } from "../api/types";

export type ImportPhase = "idle" | "running" | "finished";

export interface ImportState {
  phase: ImportPhase;
  progress: ImportProgress | null;
  result: ImportResult | null;
  /** What is being read: the card label, a path, or "sample data". */
  source: string;
  /** Whether there were no nights before this import started (first launch). */
  first: boolean;
  error: string | null;
}

export interface AppData {
  info: AppInfo | null;
  nights: NightListItem[];
  history: SettingsChange[];
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  imp: ImportState;
  /** Starts an import. `path` null loads the bundled sample card. */
  startImport: (path: string | null, label: string) => Promise<void>;
  cancelImport: () => Promise<void>;
  resetImport: () => void;
}

const Ctx = createContext<AppData | null>(null);
const IDLE: ImportState = { phase: "idle", progress: null, result: null, source: "", first: false, error: null };

/** App-wide data and import state. Import listeners live here so an import survives navigation. */
export function DataProvider({ children }: { children: ReactNode }) {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [nights, setNights] = useState<NightListItem[]>([]);
  const [history, setHistory] = useState<SettingsChange[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [imp, setImp] = useState<ImportState>(IDLE);
  const running = useRef(false);
  const infoRef = useRef<AppInfo | null>(null);
  infoRef.current = info;

  const refresh = useCallback(async () => {
    try {
      const [i, n, h] = await Promise.all([api.appInfo(), api.listNights(), api.settingsHistory()]);
      setInfo(i);
      setNights(n);
      setHistory(h);
      setError(null);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);

  useEffect(() => {
    let alive = true;
    const offs: (() => void)[] = [];
    const keep = (u: () => void) => { if (alive) offs.push(u); else u(); };
    void api.onImportProgress((p) => {
      if (!alive) return;
      setImp((s) => (s.phase === "finished" ? s : { ...s, phase: "running", progress: p }));
    }).then(keep);
    void api.onImportFinished((r) => {
      if (!alive) return;
      running.current = false;
      setImp((s) => ({ ...s, phase: "finished", result: r }));
      void refresh();
    }).then(keep);
    return () => { alive = false; offs.forEach((u) => u()); };
  }, [refresh]);

  const startImport = useCallback(async (path: string | null, label: string) => {
    if (running.current) return;
    running.current = true;
    setImp({ phase: "running", progress: null, result: null, source: label, first: (infoRef.current?.nightCount ?? 0) === 0, error: null });
    try {
      if (path === null) await api.loadSampleData();
      else await api.importCard(path);
    } catch (e) {
      running.current = false;
      setImp({ ...IDLE, error: errorText(e) });
    }
  }, []);

  const cancelImport = useCallback(async () => {
    try { await api.cancelImport(); } catch (e) { setImp((s) => ({ ...s, error: errorText(e) })); }
  }, []);

  const resetImport = useCallback(() => { if (!running.current) setImp(IDLE); }, []);

  return (
    <Ctx.Provider value={{ info, nights, history, loading, error, refresh, imp, startImport, cancelImport, resetImport }}>
      {children}
    </Ctx.Provider>
  );
}

export function useData(): AppData {
  const v = useContext(Ctx);
  if (!v) throw new Error("useData outside DataProvider");
  return v;
}

export function errorText(e: unknown): string {
  return e instanceof Error ? e.message : typeof e === "string" ? e : "Something went wrong.";
}
