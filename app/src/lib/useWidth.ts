import { useEffect, useRef, useState, type RefObject } from "react";

/** Measures an element's width (for SVG charts that draw in real pixels). */
export function useWidth<T extends HTMLElement>(fallback = 320): [RefObject<T>, number] {
  const ref = useRef<T>(null);
  const [w, setW] = useState(fallback);
  useEffect(() => {
    const el = ref.current;
    // Run after each commit: a loading state may mount the target later.
    if (!el) return;
    const ro = new ResizeObserver(() => setW(Math.max(120, Math.round(el.clientWidth))));
    ro.observe(el);
    setW(Math.max(120, Math.round(el.clientWidth)));
    return () => ro.disconnect();
  });
  return [ref, w];
}
