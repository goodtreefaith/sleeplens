import { useEffect } from "react";

export const GLOSSARY: { id: string; term: string; body: string[]; also?: string }[] = [
  { id: "hours", term: "Hours used", body: [
    "The time the machine ran with the mask on, added up across the night. Breaks with the mask off are not counted.",
    "A night with more than 4 hours is a \"valid night\". Many insurers look for valid nights on at least 70% of days (compliance)."
  ], also: "General guide: 7 hours or more is good, 4 to 7 is okay, under 4 is short." },
  { id: "ahi", term: "Events per hour (AHI)", body: [
    "AHI stands for apnea-hypopnea index: the number of apneas and hypopneas per hour of use.",
    "On therapy, under 5 an hour is generally considered the normal range. 5 to 15 is worth watching, and over 15 is worth raising with your clinician."
  ], also: "General guide only. Your clinician may set a different goal for you." },
  { id: "obstructive", term: "Obstructive apnea", body: [
    "Breathing stopped for 10 seconds or more because the airway narrowed or closed. CPAP pressure is meant to hold the airway open, so an auto machine usually raises pressure after these."
  ] },
  { id: "central", term: "Central apnea", body: [
    "Breathing stopped for 10 seconds or more while the airway was open: the body briefly did not take a breath. Raising pressure does not help these, so the machine does not respond the same way."
  ] },
  { id: "hypopnea", term: "Hypopnea", body: [
    "Breathing became much shallower (but did not stop) for 10 seconds or more."
  ] },
  { id: "leak", term: "Mask leak", body: [
    "Every CPAP mask has a vent that lets out the air you breathe out. That is intended leak. Extra leak is air escaping beyond the vent, usually around the mask edge or through an open mouth.",
    "SleepLens shows the median extra leak and the minutes of large leak (over 90 L/min). General guide: large leak under 5% of the night is good, 5 to 20% is worth watching."
  ], also: "Large leaks make event counts less reliable, and can dry the mouth or wake you." },
  { id: "pressure", term: "Pressure", body: [
    "The air pressure the machine delivered. An auto CPAP moves within the minimum and maximum your clinician set, rising when it senses the airway narrowing.",
    "\"Average\" is the mean across the night. \"High (90%)\" is the level pressure stayed under 90% of the time. If it often sits at the top of the range, mention it at your next visit."
  ] },
  { id: "units", term: "hPa and cmH₂O", body: [
    "Pressure units. Some machines show hectopascals (hPa), others centimetres of water (cmH₂O). 1 hPa is about 1.02 cmH₂O, so the numbers are almost the same. SleepLens uses the unit your machine is set to."
  ] },
  { id: "ramp", term: "Ramp", body: [
    "A comfort feature: the machine starts at a lower pressure and rises slowly over a set time (for example 15 minutes) while you fall asleep."
  ] },
  { id: "comfort", term: "Comfort level (exhale relief)", body: [
    "Lowers the pressure a little each time you breathe out, so exhaling feels easier. ResMed calls this EPR. Higher levels mean more relief."
  ] },
  { id: "humidifier", term: "Humidifier", body: [
    "Warms and moistens the air to prevent a dry mouth or nose. The level is a comfort setting."
  ] },
  { id: "computed", term: "Computed by SleepLens", body: [
    "SleepLens prefers the values your machine stored in its own summary, because they match the machine's screen. When the machine did not store one (for example nights older than its summary log keeps), SleepLens works it out from the night file and labels it."
  ] }
];

export function LearnScreen({ term }: { term?: string }) {
  useEffect(() => {
    if (!term) return;
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
