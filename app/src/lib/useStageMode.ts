import { useEffect, useState } from "react";
import { readStageMode, STAGE_MODE_STORAGE_KEY, type StageMode } from "./stages";

const MODE_EVENT = "sleeplens-stage-mode-change";
export function useStageMode() {
  const [mode, setMode] = useState<StageMode>(readStageMode);
  useEffect(() => {
    const sync = (event: Event) => setMode(event instanceof CustomEvent ? event.detail : readStageMode());
    window.addEventListener(MODE_EVENT, sync); window.addEventListener("storage", sync);
    return () => { window.removeEventListener(MODE_EVENT, sync); window.removeEventListener("storage", sync); };
  }, []);
  function change(next: StageMode) {
    setMode(next);
    try { window.localStorage.setItem(STAGE_MODE_STORAGE_KEY, next); } catch { /* The view still works when storage is unavailable. */ }
    window.dispatchEvent(new CustomEvent(MODE_EVENT, { detail: next }));
  }
  return [mode, change] as const;
}
