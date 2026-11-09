import { useEffect } from "react";

import { GLOSSARY, resolveGlossaryKey } from "../lib/glossary";

export function LearnScreen({ term }: { term?: string }) {
  useEffect(() => {
    if (!term) return;
    term = resolveGlossaryKey(term);
    const el = document.getElementById(`learn-${term}`);
    if (el) {
      el.scrollIntoView({ block: "start" });
      el.focus({ preventScroll: true });
    }
  }, [term]);
  return (
    <section aria-labelledby="h-learn">
      <div className="screen-h"><h1 id="h-learn">Learn</h1><p className="sub">Plain-language explanations of the words SleepLens uses.</p></div>
      <div className="glossary">
        {GLOSSARY.map((g) => (
          <article
            key={g.id}
            id={`learn-${g.id}`}
            tabIndex={-1}
            className="card"
            aria-labelledby={`lh-${g.id}`}
            style={term === g.id ? { borderColor: "var(--accent)", boxShadow: "0 0 0 2px var(--accent)" } : undefined}
          >
            <h2 id={`lh-${g.id}`}>{g.term}</h2>
            {g.body.map((p, i) => <p key={i}>{p}</p>)}
            {g.also ? <p className="also">{g.also}</p> : null}
          </article>
        ))}
      </div>
      <p className="foot-note">Ranges are general guides drawn from common clinical practice (for example the AASM scoring rules and insurer compliance rules), not targets set for you. SleepLens is not medical advice. Talk to your clinician before changing anything.</p>
    </section>
  );
}
