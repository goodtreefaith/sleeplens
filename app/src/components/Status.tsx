import type { ReactNode } from "react";
import type { Status } from "../lib/status";
import { Icon } from "./Icon";

/** Status is always word + icon + colour. */
export function StatusPill({ s, word }: { s: Status; word: string }) {
  return (
    <p className={`status ${s}`}>
      <Icon name={s} />
      <span>{word}</span>
    </p>
  );
}

export function SourceTag({ computed }: { computed: boolean }) {
  if (!computed) return null;
  return (
    <span className="src" title="The machine did not store this value, so SleepLens worked it out from the night file.">
      Computed by SleepLens
    </span>
  );
}

export interface Zone { k: "good" | "watch" | "talk" | "band" | "none"; from: number; to: number; label: string }
export interface Marker { v: number; label?: string; cls?: string }

/** Healthy-range band behind the value (WHOOP-style). Zone labels centred in each zone; marker labels below the bar. */
export function RangeBar(o: { min: number; max: number; zones: Zone[]; markers: Marker[]; aria: string }) {
  const pct = (v: number) => Math.max(0, Math.min(100, ((v - o.min) / (o.max - o.min)) * 100));
  const labelled = o.markers.filter((m) => m.label);
  return (
    <div className="rb" role="img" aria-label={o.aria}>
      <div className="rb-track">
        {o.zones.map((z, i) => (
          <span key={i} className={`seg z-${z.k}`} style={{ left: `${pct(z.from)}%`, width: `${pct(z.to) - pct(z.from)}%` }} />
        ))}
        {o.markers.map((m, i) => (
          <span key={i} className={`mark ${m.cls ?? ""}`} style={{ left: `${pct(m.v)}%` }} />
        ))}
      </div>
      {labelled.length ? (
        <div className="rb-marks" aria-hidden="true">
          {labelled.map((m, i) => <span key={i} style={{ left: `${pct(m.v)}%` }}>{m.label}</span>)}
        </div>
      ) : null}
      <div className="rb-labels" aria-hidden="true">
        {o.zones.map((z, i) => (
          <span key={i} style={{ left: `${pct(z.from)}%`, width: `${pct(z.to) - pct(z.from)}%` }}>{z.label}</span>
        ))}
      </div>
    </div>
  );
}

export function What({ children, learn, onLearn }: { children: ReactNode; learn?: string; onLearn?: (term: string) => void }) {
  return (
    <details className="what">
      <summary>What this means</summary>
      {children}
      {learn && onLearn ? (
        <p>
          <a href={`#learn/${learn}`} onClick={(e) => { e.preventDefault(); onLearn(learn); }}>More in Learn</a>
        </p>
      ) : null}
    </details>
  );
}
