import { useEffect, useRef, useState } from "react";
import type { Go } from "../App";
import { api } from "../api";
import { clearRhythmProfile, EMPTY_RHYTHM, focusOverlapsSleep, readRhythmProfile, rhythmErrors, rhythmLabel, saveRhythmProfile, windowMinutes, type RhythmDraft } from "../lib/rhythm";
import { hm } from "../lib/format";
import "../rhythm.css";

const PREFERENCES = [
  ["earlier", "Earlier in the day", "I tend to prefer an early start."],
  ["middle", "The middle of the day", "I feel most drawn to daytime hours."],
  ["later", "Later in the day", "I tend to prefer a later start."],
  ["varies", "It varies", "I do not have a clear preference."]
] as const;
const STEP_NAMES = ["Your preference", "Your free-day timing", "Your focus window"];

function timeWindow(start: string, end: string): string {
  return `${start} to ${end}`;
}

export function RhythmScreen({ go }: { go: Go }) {
  const persistent = api.kind === "tauri";
  const questionHeading = useRef<HTMLHeadingElement>(null);
  const pageHeading = useRef<HTMLHeadingElement>(null);
  const resultHeading = useRef<HTMLHeadingElement>(null);
  const clearDialog = useRef<HTMLDialogElement>(null);
  const [profile, setProfile] = useState(() => readRhythmProfile(persistent));
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState<RhythmDraft>(() => profile ? { ...profile } : { ...EMPTY_RHYTHM });
  const [step, setStep] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const update = <K extends keyof RhythmDraft>(key: K, value: RhythmDraft[K]) => { setError(null); setDraft(previous => ({ ...previous, [key]: value })); };
  const errors = rhythmErrors(draft, step);
  useEffect(() => {
    if (editing) questionHeading.current?.focus();
    else (profile ? resultHeading.current : pageHeading.current)?.focus();
  }, [editing, step, profile]);

  function notNow() {
    setEditing(false); setError(null);
    if (!profile) {
      go({ screen: "overview" });
      requestAnimationFrame(() => {
        const heading = document.querySelector<HTMLElement>("#content h1");
        if (heading) { heading.tabIndex = -1; heading.focus(); }
      });
    }
  }

  function save() {
    try { setProfile(saveRhythmProfile(draft, persistent)); setEditing(false); setError(null); }
    catch (e) { setError(e instanceof Error ? e.message : "The answers could not be saved."); }
  }
  function clear() {
    try { clearRhythmProfile(persistent); setProfile(null); setDraft({ ...EMPTY_RHYTHM }); setEditing(false); setStep(0); setError(null); clearDialog.current?.close(); }
    catch { setError("The saved answers could not be cleared. Try again before closing the app."); }
  }
  function begin() { setDraft(profile ? { ...profile } : { ...EMPTY_RHYTHM }); setStep(0); setError(null); setEditing(true); }

  return <section className="rhythm-screen" aria-labelledby="rhythm-heading">
    <div className="screen-h"><div><h1 id="rhythm-heading" ref={pageHeading} tabIndex={-1}>Your rhythm</h1><p className="sub">An optional look at the timing you prefer.</p></div></div>
    <div className="rhythm-layout"><div>
      {!editing && !profile ? <section className="card rhythm-intro">
        <h2>Make room for your own schedule</h2>
        <p>Answer a few questions about your day preference, free-day sleep timing and a focus window you would like to try.</p>
        <p>This short reflection is not a validated chronotype test. It does not measure your body clock or predict when you perform best.</p>
        <div className="actions"><button type="button" className="btn primary" onClick={begin}>Explore my rhythm</button><button type="button" className="btn" onClick={notNow}>Not now</button></div>
      </section> : null}
      {!editing && profile ? <section className="card rhythm-result" aria-labelledby="rhythm-result-heading">
        <p className="quiet">Your reported rhythm</p><h2 id="rhythm-result-heading" ref={resultHeading} tabIndex={-1}>{rhythmLabel(profile.preference)}</h2>
        <p>This reflects your answers, not a measurement of your body clock.</p>
        {profile.schedule === "variable" || profile.alarm === "constrained" ? <p className="banner info" role="note">{profile.schedule === "variable" ? "Your variable or shift schedule can change the timing that works for you. " : ""}{profile.alarm === "constrained" ? "Alarms or obligations limit your free-day timing, so it may not reflect your preferred schedule." : ""}</p> : null}
        {focusOverlapsSleep(profile) ? <p className="banner info" role="note">Your focus window overlaps your reported sleep window. Adjust either window if you plan to use them on the same day.</p> : null}
        <dl className="rhythm-facts"><div><dt>Your usual free-day sleep window</dt><dd>{timeWindow(profile.bedtime, profile.wakeTime)}<small>{profile.wakeTime < profile.bedtime ? "Ends the next day. " : ""}{hm(windowMinutes(profile.bedtime, profile.wakeTime)!)} in your reported window. This is not measured sleep.</small></dd></div><div><dt>Your chosen focus window</dt><dd>{timeWindow(profile.focusStart, profile.focusEnd)}<small>{profile.focusEnd < profile.focusStart ? "Ends the next day. " : ""}A planning idea you chose. SleepLens did not measure a peak-performance window.</small></dd></div></dl>
        <p className="quiet">Try this window when your schedule allows, then adjust it based on your own experience. Your answers do not change your sleep score, stage estimates or therapy advice.</p>
        <div className="actions"><button type="button" className="btn" onClick={begin}>Edit answers</button><button type="button" className="btn" onClick={() => clearDialog.current?.showModal()}>Clear answers</button></div>
      </section> : null}
      {editing ? <form className="card rhythm-question" onSubmit={e => { e.preventDefault(); if (errors.length) return; if (step < 2) setStep(step + 1); else save(); }}>
        <div className="rhythm-question-top"><p className="quiet">Question {step + 1} of 3</p><button type="button" className="btn link" onClick={notNow}>Not now</button></div>
        <h2 ref={questionHeading} tabIndex={-1}>{STEP_NAMES[step]}</h2>
        {step === 0 ? <fieldset><legend>On a day without work deadlines, which timing do you tend to prefer?</legend><div className="rhythm-choices">{PREFERENCES.map(([value, label, description]) => <label className="rhythm-choice" key={value}><input type="radio" name="preference" value={value} checked={draft.preference === value} onChange={() => update("preference", value)} /><span><strong>{label}</strong><small>{description}</small></span></label>)}</div></fieldset> : null}
        {step === 1 ? <><p>Think about a typical day without work or school. Use your usual local clock times; an overnight window is fine.</p><div className="rhythm-times"><label>Usual bedtime<input type="time" required value={draft.bedtime} onChange={e => update("bedtime", e.target.value)} /></label><label>Usual wake time<input type="time" required value={draft.wakeTime} onChange={e => update("wakeTime", e.target.value)} /></label></div>{windowMinutes(draft.bedtime, draft.wakeTime) != null ? <p className="quiet">{hm(windowMinutes(draft.bedtime, draft.wakeTime)!)} in this reported window{draft.wakeTime < draft.bedtime ? ", ending the next day" : ""}.</p> : null}
          <fieldset><legend>How steady is your schedule?</legend><div className="rhythm-choices">{[["regular", "Usually regular"], ["variable", "Variable or shift work"]].map(([value, label]) => <label className="rhythm-choice" key={value}><input type="radio" name="schedule" checked={draft.schedule === value} onChange={() => update("schedule", value as "regular" | "variable")} /><span>{label}</span></label>)}</div></fieldset>
          <fieldset><legend>Do alarms or other obligations limit that free-day timing?</legend><div className="rhythm-choices">{[["free", "Usually no"], ["constrained", "Yes, they often do"]].map(([value, label]) => <label className="rhythm-choice" key={value}><input type="radio" name="alarm" checked={draft.alarm === value} onChange={() => update("alarm", value as "free" | "constrained")} /><span>{label}</span></label>)}</div></fieldset></> : null}
        {step === 2 ? <><p>Choose a time window you would like to reserve for focused work or a personal task. This is your planning choice, not a predicted productivity peak.</p><div className="rhythm-times"><label>Start time<input type="time" required value={draft.focusStart} onChange={e => update("focusStart", e.target.value)} /></label><label>End time<input type="time" required value={draft.focusEnd} onChange={e => update("focusEnd", e.target.value)} /></label></div>{windowMinutes(draft.focusStart, draft.focusEnd) != null ? <p className="quiet">{hm(windowMinutes(draft.focusStart, draft.focusEnd)!)} in your chosen focus window{draft.focusEnd < draft.focusStart ? ", ending the next day" : ""}.</p> : null}<p className="quiet">An end time before the start means the window crosses midnight. Your schedule and responsibilities take priority.</p>{focusOverlapsSleep(draft) ? <p className="banner info" role="note">This overlaps your reported sleep window. You can revise the times before saving, or keep them if they describe different days.</p> : null}</> : null}
        {errors.length ? <p className="quiet rhythm-validation" aria-live="polite">{errors[0]}</p> : null}
        <div className="actions">{step > 0 ? <button type="button" className="btn" onClick={() => setStep(step - 1)}>Back</button> : null}<button type="submit" className="btn primary" disabled={errors.length > 0}>{step === 2 ? "Save my rhythm" : "Continue"}</button></div>
      </form> : null}
      {error ? <p className="error-box" role="alert">{error}</p> : null}
    </div><aside className="card rhythm-about"><img className="rhythm-fox" src="./mascots/rhythm-fox.png" width="100" height="100" alt="" aria-hidden="true" /><h2>Your choice, kept local</h2><p>{persistent ? "Answers stay in this app on your computer. After you answer, you can clear them on this page or use Delete all data in Settings." : "Demo answers stay in memory for this page only. Reloading clears them."}</p><p>Answering is optional. SleepLens does not infer a chronotype from CPAP data, and these answers do not change therapy measurements.</p></aside></div>
    <dialog className="confirm" ref={clearDialog} aria-labelledby="clear-rhythm-heading"><h2 id="clear-rhythm-heading">Clear your rhythm answers?</h2><p>This removes your answers and personal planning window. Therapy measurements stay unchanged.</p><div className="actions"><button type="button" className="btn" autoFocus onClick={() => clearDialog.current?.close()}>Keep answers</button><button type="button" className="btn danger" onClick={clear}>Clear answers</button></div></dialog>
  </section>;
}
