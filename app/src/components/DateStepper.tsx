import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import type { NightListItem } from "../api/types";
import { addDays, dateMs } from "../lib/dates";
import { dayDate, longDate, longDateY, monthYear } from "../lib/format";
import { Icon } from "./Icon";

export function neighbours(nights: NightListItem[], d: string) {
  let prev: string | null = null;
  let next: string | null = null;
  for (const n of nights) {
    if (n.date < d) prev = n.date;
    else if (n.date > d) { next = n.date; break; }
  }
  return { prev, next };
}

/** Previous / next night arrows, a calendar popover and a "Latest night" button. */
export function DateStepper({ nights, date, onDate }: { nights: NightListItem[]; date: string; onDate: (d: string) => void }) {
  const { prev, next } = neighbours(nights, date);
  const last = nights[nights.length - 1]?.date;
  const has = nights.some((n) => n.date === date);
  const isLatest = date === last;
  const [open, setOpen] = useState(false);
  const prevBtn = useRef<HTMLButtonElement>(null);
  const nextBtn = useRef<HTMLButtonElement>(null);
  const calBtn = useRef<HTMLButtonElement>(null);
  const lastUsed = useRef<"prev" | "next" | null>(null);

  // Keep keyboard focus useful when the arrow you just used becomes disabled.
  useEffect(() => {
    if (lastUsed.current === "prev" && !prev) nextBtn.current?.focus();
    if (lastUsed.current === "next" && !next) prevBtn.current?.focus();
    lastUsed.current = null;
  }, [date, prev, next]);

  const sub = !has ? "No therapy recorded" : isLatest ? "Night starting this evening" : `Night of ${dayDate(date)}`;
  return (
    <div className="datebar" role="group" aria-label="Choose a night">
      <button ref={prevBtn} className="btn icon" type="button" disabled={!prev} onClick={() => { lastUsed.current = "prev"; if (prev) onDate(prev); }} aria-label={`Previous night${prev ? ", " + longDate(prev) : ""}`}>
        <Icon name="chevL" />
      </button>
      <div className="dlabel">
        <strong>{longDateY(date)}</strong>
        <span>{sub}</span>
      </div>
      <button ref={nextBtn} className="btn icon" type="button" disabled={!next} onClick={() => { lastUsed.current = "next"; if (next) onDate(next); }} aria-label={`Next night${next ? ", " + longDate(next) : ""}`}>
        <Icon name="chev" />
      </button>
      <div className="calwrap">
        <button ref={calBtn} className="btn icon" type="button" aria-haspopup="dialog" aria-expanded={open} aria-label="Pick a night from a calendar" onClick={() => setOpen((o) => !o)}>
          <Icon name="calendar" />
        </button>
        {open ? (
          <CalendarPopover
            nights={nights}
            date={date}
            onPick={(d) => { setOpen(false); onDate(d); calBtn.current?.focus(); }}
            onClose={() => { setOpen(false); calBtn.current?.focus(); }}
          />
        ) : null}
      </div>
      {!isLatest && last ? <button className="btn" type="button" onClick={() => onDate(last)}>Latest night</button> : null}
    </div>
  );
}

const WD = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

function monthOf(d: string) { return d.slice(0, 7); }
function shiftMonth(m: string, k: number) {
  const [y, mo] = m.split("-").map(Number) as [number, number];
  const t = new Date(Date.UTC(y, mo - 1 + k, 1));
  return t.toISOString().slice(0, 7);
}

/** Small month calendar; only nights with data can be chosen. Arrow keys move, Enter picks, Esc closes. */
function CalendarPopover({ nights, date, onPick, onClose }: { nights: NightListItem[]; date: string; onPick: (d: string) => void; onClose: () => void }) {
  const set = new Set(nights.map((n) => n.date));
  const first = nights[0]?.date ?? date, last = nights[nights.length - 1]?.date ?? date;
  const [month, setMonth] = useState(monthOf(date));
  const [focus, setFocus] = useState(date);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = root.current?.querySelector<HTMLButtonElement>(`[data-d="${focus}"]`);
    el?.focus();
  }, [focus, month]);

  useEffect(() => {
    const onDown = (e: MouseEvent) => { if (root.current && !root.current.contains(e.target as Node) && !(e.target as Element).closest?.(".calwrap")) onClose(); };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [onClose]);

  const [y, mo] = month.split("-").map(Number) as [number, number];
  const daysIn = new Date(Date.UTC(y, mo, 0)).getUTCDate();
  const lead = (new Date(dateMs(`${month}-01`)).getUTCDay() + 6) % 7;
  const cells: (string | null)[] = [...Array(lead).fill(null)];
  for (let i = 1; i <= daysIn; i++) cells.push(`${month}-${String(i).padStart(2, "0")}`);
  const title = monthYear(`${month}-01`);

  const move = (k: number) => {
    const n = addDays(focus, k);
    if (n < first.slice(0, 8) + "01" || n > last) return;
    setFocus(n);
    if (monthOf(n) !== month) setMonth(monthOf(n));
  };
  const onKey = (e: KeyboardEvent) => {
    const map: Record<string, number> = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -7, ArrowDown: 7 };
    if (e.key === "Escape") { e.preventDefault(); onClose(); }
    else if (e.key in map) { e.preventDefault(); move(map[e.key]!); }
    else if (e.key === "PageUp" || e.key === "PageDown") {
      e.preventDefault();
      const m = shiftMonth(month, e.key === "PageUp" ? -1 : 1);
      if (m >= monthOf(first) && m <= monthOf(last)) { setMonth(m); setFocus(`${m}-01` < first ? first : `${m}-01`); }
    }
  };
  const canPrev = shiftMonth(month, -1) >= monthOf(first);
  const canNext = shiftMonth(month, 1) <= monthOf(last);

  return (
    <div className="calpop" role="dialog" aria-label="Choose a night" ref={root} onKeyDown={onKey}>
      <div className="cal-h">
        <button className="btn icon sm" type="button" disabled={!canPrev} aria-label="Previous month" onClick={() => setMonth(shiftMonth(month, -1))}><Icon name="chevL" /></button>
        <b aria-live="polite">{title}</b>
        <button className="btn icon sm" type="button" disabled={!canNext} aria-label="Next month" onClick={() => setMonth(shiftMonth(month, 1))}><Icon name="chev" /></button>
      </div>
      <div className="cal-grid" role="grid">
        {WD.map((w) => <span key={w} className="cal-wd" aria-hidden="true">{w.slice(0, 2)}</span>)}
        {cells.map((d, i) => d ? (
          <button
            key={d}
            type="button"
            data-d={d}
            className={`cal-d${set.has(d) ? " has" : ""}${d === date ? " sel" : ""}`}
            tabIndex={d === focus ? 0 : -1}
            aria-disabled={!set.has(d)}
            aria-current={d === date ? "date" : undefined}
            aria-label={`${longDateY(d)}${set.has(d) ? "" : ", no recording"}`}
            onClick={() => set.has(d) && onPick(d)}
            onKeyDown={(e) => { if ((e.key === "Enter" || e.key === " ") && set.has(d)) { e.preventDefault(); onPick(d); } }}
          >{Number(d.slice(8))}</button>
        ) : <span key={"e" + i} />)}
      </div>
      <p className="cal-foot"><span className="cal-key" aria-hidden="true" />Nights with data</p>
    </div>
  );
}
