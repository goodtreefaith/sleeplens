import { useEffect } from "react";

import { GLOSSARY, GLOSSARY_GROUPS, resolveGlossaryKey } from "../lib/glossary";

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
      <nav className="learn-index" aria-label="Learn sections">{GLOSSARY_GROUPS.map(group => <a key={group.id} href={`#learn-group-${group.id}`} onClick={event => { event.preventDefault(); document.getElementById(`learn-group-${group.id}`)?.scrollIntoView({ block: "start" }); }}>{group.title} ›</a>)}</nav>
      {GLOSSARY_GROUPS.map(group => <section className="glossary-section" id={`learn-group-${group.id}`} key={group.id} aria-labelledby={`learn-group-title-${group.id}`}><h2 id={`learn-group-title-${group.id}`}>{group.title}</h2><div className="glossary">
        {group.keys.map(key => GLOSSARY.find(entry => entry.id === key)!).map((g) => (
          <article
            key={g.id}
            id={`learn-${g.id}`}
            tabIndex={-1}
            className="card"
            aria-labelledby={`lh-${g.id}`}
            style={term === g.id ? { borderColor: "var(--accent)", boxShadow: "0 0 0 2px var(--accent)" } : undefined}
          >
            <h3 id={`lh-${g.id}`}>{g.id === "sleepScore" ? "About the sleep score" : g.term}</h3>
            {g.body.map((p, i) => <p key={i}>{p}</p>)}
            {g.also ? <p className="also">{g.also}</p> : null}
          </article>
        ))}
      </div></section>)}
      <p className="foot-note">Ranges are general guides drawn from common clinical practice (for example the AASM scoring rules and insurer compliance rules), not targets set for you. SleepLens is not medical advice. Talk to your clinician before changing anything.</p>
    </section>
  );
}
