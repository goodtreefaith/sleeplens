import { useEffect, useId, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { Go } from "../App";
import { glossaryEntry, resolveGlossaryKey } from "../lib/glossary";
import "../v2.css";

export function HelpTip({ term, go }: { term: string; go?: Go }) {
  const entry = glossaryEntry(term);
  const [open, setOpen] = useState(false);
  const [position, setPosition] = useState({ top: 0, left: 0 });
  const button = useRef<HTMLButtonElement>(null);
  const popover = useRef<HTMLDivElement>(null);
  const timer = useRef<ReturnType<typeof setTimeout>>();
  const id = useId();
  function cancel() { clearTimeout(timer.current); }
  function hoverClose() { if (!popover.current?.contains(document.activeElement)) close(); }
  function close(restore = false) { cancel(); setOpen(false); if (restore) button.current?.focus(); }
  function show(focus = false) {
    cancel();
    const r = button.current?.getBoundingClientRect();
    if (r) setPosition({ left: Math.min(Math.max(12, r.left - 120), window.innerWidth - Math.min(330, window.innerWidth - 24) - 12), top: Math.max(12, Math.min(r.bottom + 8, window.innerHeight - 280)) });
    setOpen(true);
    if (focus) requestAnimationFrame(() => popover.current?.querySelector<HTMLButtonElement>("button")?.focus());
  }
  useEffect(() => {
    if (!open) return;
    const key = (e: KeyboardEvent) => { if (e.key === "Escape") { e.preventDefault(); close(true); } };
    const outside = (e: PointerEvent) => { if (!button.current?.contains(e.target as Node) && !popover.current?.contains(e.target as Node)) close(); };
    const blur = (e: FocusEvent) => { if (!button.current?.contains(e.target as Node) && !popover.current?.contains(e.target as Node)) close(); };
    const scroll = () => close();
    document.addEventListener("keydown", key); document.addEventListener("pointerdown", outside); document.addEventListener("focusin", blur); window.addEventListener("scroll", scroll, true); window.addEventListener("resize", scroll);
    return () => { document.removeEventListener("keydown", key); document.removeEventListener("pointerdown", outside); document.removeEventListener("focusin", blur); window.removeEventListener("scroll", scroll, true); window.removeEventListener("resize", scroll); };
  }, [open]);
  useEffect(() => () => cancel(), []);
  if (!entry) return null;
  return <span className="help-wrap" onMouseEnter={() => { cancel(); timer.current = setTimeout(() => show(), 350); }} onMouseLeave={() => { cancel(); timer.current = setTimeout(hoverClose, 250); }}>
    <button ref={button} type="button" className="help-tip" aria-label={`What is ${entry.term}?`} aria-expanded={open} aria-controls={open ? id : undefined} onClick={() => open ? close(true) : show(true)}>?</button>
    {open && createPortal(<div ref={popover} id={id} role="dialog" aria-label={entry.term} className="help-pop" style={{ ...position, maxHeight: `calc(100vh - ${position.top + 12}px)` }} onMouseEnter={cancel} onMouseLeave={() => { timer.current = setTimeout(hoverClose, 250); }}>
      <div className="help-head"><strong>{entry.term}</strong><button type="button" aria-label="Close explanation" onClick={() => close(true)}>×</button></div>
      <p>{entry.body.join(" ")}</p>
      <a href={`#learn/${resolveGlossaryKey(term)}`} onClick={(e) => { if (go) { e.preventDefault(); go({ screen: "learn", term: resolveGlossaryKey(term) }); } close(); }}>More in Learn</a>
    </div>, document.body)}
  </span>;
}
