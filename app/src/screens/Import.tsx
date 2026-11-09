import { useEffect, useState, type DragEvent } from "react";
import { api } from "../api";
import type { CardCandidate, ImportProgress, ImportResult } from "../api/types";
import type { Go } from "../App";
import { Icon } from "../components/Icon";
import { useData, errorText } from "../lib/data";
import type { MonthStatus } from "../lib/importMonths";
import "../score-import.css";
import { monthYear, plural, shortDateY } from "../lib/format";

const STEPS: { phase: ImportProgress["phase"]; label: string }[] = [
  { phase: "scanning", label: "Finding nights on the card" },
  { phase: "summaries", label: "Reading the machine's summary log" },
  { phase: "nights", label: "Copying nights to this computer" }
];
const ORDER = ["scanning", "summaries", "nights", "done"];

export function importPercent(p: ImportProgress | null): number {
  if (!p) return 2;
  if (p.phase === "done") return 100;
  if (p.phase === "nights" && p.total > 0) return Math.max(8, Math.round((p.done / p.total) * 100));
  return p.phase === "summaries" ? 6 : 3;
}

export function ImportScreen({ go }: { go: Go }) {
  const { info, imp, startImport, cancelImport, resetImport } = useData();
  const [cards, setCards] = useState<CardCandidate[]>([]);
  const [over, setOver] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    api.findCards().then((c) => alive && setCards(c)).catch(() => alive && setCards([]));
    return () => { alive = false; };
  }, []);

  const start = (path: string | null, label: string) => { setErr(null); void startImport(path, label); };

  async function choose() {
    setErr(null);
    try {
      const p = await api.pickFolder();
      if (p) start(p, p);
    } catch (e) {
      setErr(errorText(e));
    }
  }

  // Window drag and drop: real paths inside Tauri; HTML drag events in a plain browser.
  useEffect(() => {
    let off: (() => void) | null = null;
    let alive = true;
    void api.onDragDrop((e) => {
      if (e.type === "enter" || e.type === "over") setOver(true);
      else if (e.type === "leave") setOver(false);
      else {
        setOver(false);
        const p = e.paths[0];
        if (p) start(p, p);
      }
    }).then((u) => { if (alive) off = u; else u?.(); });
    return () => { alive = false; off?.(); };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const dropProps = api.kind === "mock"
    ? {
        onDragOver: (e: DragEvent) => { e.preventDefault(); setOver(true); },
        onDragLeave: () => setOver(false),
        onDrop: (e: DragEvent) => {
          e.preventDefault();
          setOver(false);
          const name = e.dataTransfer.files[0]?.name ?? "NO NAME";
          start(`/Volumes/${name}`, name);
        }
      }
    : {};

  const card = cards[0];
  const progress = imp.progress;
  const activeCandidate = cards.find(candidate => candidate.label === imp.source || candidate.path === imp.source);
  const months = [...new Set([...(activeCandidate?.months ?? []), ...Object.keys(imp.months)])].sort();
  const monthStatus = (month: string): MonthStatus => {
    if (imp.phase === "finished" && imp.result?.status === "ok") {
      const r = imp.result;
      return r.nightsSkipped > 0 && !r.nightsAdded && !r.nightsUpdated && !r.nightsFailed.length ? "Skipped — already imported" : "Done";
    }
    return imp.months[month] ?? "Waiting";
  };
  const monthList = months.length ? <ul className="import-months" aria-label="Month folder progress">{months.map(month => {
    const status = monthStatus(month);
    const failed = imp.result?.nightsFailed.filter(night => night.date.startsWith(month)).length ?? 0;
    return <li key={month} className={status === "Reading…" ? "reading" : status === "Done" ? "done" : ""}><b>{monthYear(`${month}-01`)}</b><span><Icon name={status === "Done" || status.startsWith("Skipped") ? "check" : status === "Reading…" ? "dot" : "none"} />{status}{failed > 0 ? ` · ${plural(failed, "night")} could not be read` : ""}</span></li>;
  })}</ul> : <p className="quiet">Month folders will appear as nights are found.</p>;
  const stepIndex = progress ? ORDER.indexOf(progress.phase) : 0;
  const isFirst = (info?.nightCount ?? 0) === 0;
  const pctDone = importPercent(progress);
  const shownErr = err ?? imp.error;

  return (
    <div className="import-wrap">
      <section className="lead" aria-labelledby="h-import">
        <div className="import-head">
          <h1 id="h-import">{isFirst ? "Understand your CPAP data" : "Import a card"}</h1>
          <p className="sub">Choose your SD card. Your data is copied to this computer and never leaves it.</p>
        </div>

        {imp.phase === "idle" ? (
          <>
            <div className={`drop${over ? " over" : ""}`} {...dropProps}>
              <Icon name="card" className="big" />
              <h2>{over ? "Drop to read the card" : "Drag your SD card or its folder onto this window"}</h2>
              <p>SleepLens reads the card, copies what it needs and leaves the card unchanged. Importing again later only reads new nights.</p>
              <div className="actions">
                <button className="btn primary" type="button" onClick={choose}><Icon name="folder" />Choose SD card…</button>
                <button className="btn link" type="button" onClick={() => start(null, "sample data")}>Try with sample data</button>
              </div>
            </div>
            {card ? (
              <div className="found" role="status">
                <Icon name="card" />
                <div className="grow">
                  <b>Card found: {card.label}</b>
                  <p>{plural(card.nights, "night")} from {monthYear(card.months[0] + "-01")} to {monthYear(card.months[card.months.length - 1] + "-01")}. Aeonmed AS2X format.</p>
                </div>
                <button className="btn primary" type="button" onClick={() => start(card.path, card.label)}>Read this card</button>
              </div>
            ) : null}
            {shownErr ? <p className="error-box" role="alert" style={{ marginTop: 16 }}>{shownErr}</p> : null}
          </>
        ) : null}

        {imp.phase === "running" ? (
          <section className="card import-card" aria-labelledby="h-reading" aria-busy="true">
            <div className="card-h"><h2 id="h-reading">Reading {imp.source}</h2><span className="grow" />
              <button className="btn" type="button" onClick={() => void cancelImport()}>Cancel</button></div>
            <div className="progress" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={pctDone} aria-label="Import progress">
              <span style={{ width: `${pctDone}%` }} />
            </div>
            <p className="quiet" aria-live="polite">
              {progress?.phase === "nights"
                ? `${progress.done} of ${progress.total} nights${progress.currentDate ? ` · ${shortDateY(progress.currentDate)}` : ""}`
                : progress?.message ?? "Starting…"}
            </p>
            {monthList}
            <ol className="steps">
              {STEPS.map((s, i) => {
                const state = stepIndex > i ? "ok" : stepIndex === i ? "now" : "";
                return (
                  <li key={s.phase} className={state}>
                    <Icon name={state === "ok" ? "check" : state === "now" ? "dot" : "none"} />
                    {s.label}{state === "ok" ? " · done" : ""}
                  </li>
                );
              })}
            </ol>
            <p className="quiet" style={{ marginTop: 12 }}>You can keep using SleepLens while this runs.</p>
          </section>
        ) : null}

        {imp.phase === "finished" && imp.result ? <><Result result={imp.result} first={imp.first} go={go} onAgain={resetImport} />{months.length > 0 && <section className="card import-card" aria-label="Month folder results">{monthList}</section>}</> : null}
      </section>

      <aside className="aside" aria-label="About importing">
        <section className="card" aria-labelledby="h-machines">
          <div className="card-h"><h2 id="h-machines">Supported machines</h2></div>
          <ul className="machines">
            <li className="yes"><Icon name="good" /><span><b>Aeonmed AS100A</b> (cards with an <code>AS2X</code> folder)</span></li>
            <li className="later"><Icon name="none" /><span>ResMed AirSense 10 and 11 · planned</span></li>
            <li className="later"><Icon name="none" /><span>Philips DreamStation, Luna, Resvent · help wanted</span></li>
          </ul>
        </section>
        <section className="card" aria-labelledby="h-private">
          <div className="card-h"><h2 id="h-private">Private by design</h2></div>
          <ul className="notes">
            <li className="note"><Icon name="info" /><span>No account, no internet connection. Nothing is uploaded.</span></li>
            <li className="note"><Icon name="info" /><span>Your nights are kept in one file on this computer. You can see it or delete it in Settings.</span></li>
          </ul>
        </section>
      </aside>
    </div>
  );
}

function Result({ result, first, go, onAgain }: { result: ImportResult; first: boolean; go: Go; onAgain: () => void }) {
  if (result.status === "error") {
    return (
      <section className="card import-card" aria-labelledby="h-res">
        <div className="card-h"><h2 id="h-res">This card could not be read</h2></div>
        <p className="banner"><Icon name="watch" /><span>{result.message ?? "No supported CPAP data was found."}</span></p>
        <p className="quiet" style={{ marginTop: 12 }}>SleepLens reads Aeonmed AS100A cards today. If your machine is not supported yet, you can ask for it on the project page with the folder names shown above (no therapy data needed).</p>
        <div className="row-actions"><button className="btn primary" type="button" onClick={onAgain}>Choose another folder</button></div>
      </section>
    );
  }
  if (result.status === "cancelled") {
    return (
      <section className="card import-card" aria-labelledby="h-res">
        <div className="card-h"><h2 id="h-res">Import cancelled</h2></div>
        <p className="quiet">{first ? "Nothing was imported." : "Nothing was changed. The nights you imported before are still here."}</p>
        <div className="row-actions"><button className="btn primary" type="button" onClick={onAgain}>Start again</button></div>
      </section>
    );
  }
  const failed = result.nightsFailed;
  const figs = [
    { n: result.nightsAdded, l: "new nights" },
    { n: result.nightsUpdated, l: "updated" },
    { n: result.nightsSkipped, l: "already here, unchanged" }
  ].filter((f) => f.n > 0);
  return (
    <section className="card import-card" aria-labelledby="h-res">
      <div className="card-h"><h2 id="h-res">Done. Your nights are on this computer.</h2></div>
      <p className="banner info"><Icon name="good" /><span><b>You can put the card back in your machine.</b> SleepLens does not need it any more.</span></p>
      {figs.length ? (
        <div className="result-figs">
          {figs.map((f) => <div key={f.l}><b>{f.n}</b><span>{f.l}</span></div>)}
        </div>
      ) : <p className="quiet" style={{ margin: "14px 0" }}>No new nights on this card.</p>}
      {failed.length ? (
        <div className="banner" role="note">
          <Icon name="watch" />
          <div>
            <span>{plural(failed.length, "night")} could not be read and {failed.length === 1 ? "was" : "were"} skipped: {failed.map((f) => shortDateY(f.date)).join(", ")}.</span>
            <details className="tech"><summary>Details</summary><ul>{failed.map((f) => <li key={f.date}>{shortDateY(f.date)}: {f.reason}</li>)}</ul></details>
          </div>
        </div>
      ) : null}
      <div className="row-actions">
        <button className="btn primary" type="button" onClick={() => go({ screen: "overview" })}>See last night</button>
        <button className="btn" type="button" onClick={onAgain}>Import another card</button>
      </div>
    </section>
  );
}
