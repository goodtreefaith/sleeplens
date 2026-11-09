import type { ReactNode } from "react";

// Line icons from the demo design (24 or 20 unit boxes, stroke = currentColor).
const P: Record<string, { box: number; w?: number; body: ReactNode }> = {
  good: { box: 20, w: 2, body: <><circle cx="10" cy="10" r="8" /><path d="M6.3 10.4l2.5 2.5 5-5.3" /></> },
  watch: { box: 20, w: 2, body: <><circle cx="10" cy="10" r="8" /><path d="M10 6v5M10 14v.2" /></> },
  talk: { box: 20, w: 2, body: <><path d="M3 4.5h14v9H9l-4 3v-3H3z" /><path d="M10 7v3M10 12v.2" /></> },
  none: { box: 20, w: 2, body: <><circle cx="10" cy="10" r="8" /><path d="M6.5 10h7" /></> },
  info: { box: 20, w: 1.8, body: <><circle cx="10" cy="10" r="8" /><path d="M10 9v5M10 6.2v.2" /></> },
  clock: { box: 24, body: <><circle cx="12" cy="12" r="9" /><path d="M12 7v5l3 2" /></> },
  lungs: { box: 24, body: <path d="M3 12h3l2-5 4 10 3-7 2 2h4" /> },
  wave: { box: 24, body: <path d="M3 12h3l2-5 4 10 3-7 2 2h4" /> },
  mask: { box: 24, body: <><path d="M12 3c4 0 7 3.5 7 8.5S16 21 12 21s-7-4.5-7-9.5S8 3 12 3z" /><path d="M9 12.5c1.8 1.3 4.2 1.3 6 0" /></> },
  gauge: { box: 24, body: <><path d="M4 17a8 8 0 1 1 16 0" /><path d="M12 17l4-5" /></> },
  trend: { box: 24, body: <path d="M4 20V10M10 20V4M16 20v-7M22 20H2" /> },
  chev: { box: 24, w: 2.2, body: <path d="M9 6l6 6-6 6" /> },
  chevL: { box: 24, w: 2.2, body: <path d="M15 6l-6 6 6 6" /> },
  sun: { box: 24, body: <><circle cx="12" cy="12" r="4" /><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" /></> },
  moon: { box: 24, body: <path d="M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z" /> },
  close: { box: 24, w: 2, body: <path d="M6 6l12 12M18 6L6 18" /> },
  card: { box: 24, body: <><path d="M7 3h8l4 4v14H7z" /><path d="M10 7v3M13 7v3M16 9v1" /></> },
  folder: { box: 24, body: <path d="M3 7h6l2 2h10v10H3z" /> },
  download: { box: 24, body: <><path d="M12 4v11M7 10l5 5 5-5" /><path d="M4 20h16" /></> },
  gear: { box: 24, body: <><circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z" /></> },
  book: { box: 24, body: <><path d="M4 5a2 2 0 0 1 2-2h13v16H6a2 2 0 0 0-2 2z" /><path d="M4 21V5M8 7h7" /></> },
  home: { box: 24, body: <><path d="M3 11l9-7 9 7" /><path d="M5 10v10h14V10" /></> },
  check: { box: 24, w: 2.2, body: <path d="M5 12.5l4.5 4.5L19 7.5" /> },
  dot: { box: 24, body: <circle cx="12" cy="12" r="3" /> },
  zoomOut: { box: 24, body: <><circle cx="11" cy="11" r="7" /><path d="M8 11h6M20 20l-4-4" /></> },
  sliders: { box: 24, body: <path d="M4 6h10M18 6h2M4 12h4M12 12h8M4 18h12M20 18h0M14 4v4M8 10v4M16 16v4" /> },
  trash: { box: 24, body: <><path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13" /></> },
  calendar: { box: 24, body: <><rect x="3.5" y="5" width="17" height="15" rx="2" /><path d="M3.5 10h17M8 3v4M16 3v4" /></> },
  monitor: { box: 24, body: <><rect x="3" y="4" width="18" height="12" rx="2" /><path d="M8 20h8M12 16v4" /></> }
};

export type IconName = keyof typeof P;

export function Icon({ name, className, title }: { name: IconName | string; className?: string; title?: string }) {
  const p = P[name] ?? P.dot!;
  return (
    <svg
      viewBox={`0 0 ${p.box} ${p.box}`}
      fill="none"
      stroke="currentColor"
      strokeWidth={p.w ?? 1.9}
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      aria-hidden={title ? undefined : true}
      role={title ? "img" : undefined}
    >
      {title ? <title>{title}</title> : null}
      {p.body}
    </svg>
  );
}

export function Logo({ size = 30 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" aria-hidden="true">
      <circle cx="16" cy="16" r="13" fill="none" stroke="var(--accent)" strokeWidth="3" />
      <path d="M7 17h4l2.5-5 4 9 2.5-4H25" fill="none" stroke="var(--accent)" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

/** Event marker shapes (shape + colour, so colour is never the only cue). */
export const EVENT_STYLE = {
  obstructiveApnea: { name: "Obstructive apnea", short: "Obstructive", color: "var(--ev1)", shape: "circle" },
  centralApnea: { name: "Central apnea", short: "Central", color: "var(--ev2)", shape: "diamond" },
  hypopnea: { name: "Hypopnea (shallow breathing)", short: "Hypopnea", color: "var(--evh)", shape: "triangle" },
  largeLeak: { name: "Large leak", short: "Large leak", color: "var(--leak-line)", shape: "square" }
} as const;
export type MarkerKind = keyof typeof EVENT_STYLE;

export function shapePath(shape: string, cx: number, cy: number, r: number): string {
  if (shape === "diamond") return `M${cx} ${cy - r * 1.25}L${cx + r * 1.25} ${cy}L${cx} ${cy + r * 1.25}L${cx - r * 1.25} ${cy}Z`;
  if (shape === "triangle") return `M${cx} ${cy - r * 1.2}L${cx + r * 1.2} ${cy + r * 0.9}L${cx - r * 1.2} ${cy + r * 0.9}Z`;
  if (shape === "square") return `M${cx - r} ${cy - r}h${2 * r}v${2 * r}h${-2 * r}Z`;
  return `M${cx - r} ${cy}a${r} ${r} 0 1 0 ${2 * r} 0a${r} ${r} 0 1 0 ${-2 * r} 0`;
}

export function Swatch({ kind }: { kind: MarkerKind }) {
  const s = EVENT_STYLE[kind];
  return (
    <svg viewBox="0 0 14 14" aria-hidden="true">
      <path d={shapePath(s.shape, 7, 7, 5)} fill={s.color} stroke="var(--surface)" strokeWidth="1.5" />
    </svg>
  );
}
