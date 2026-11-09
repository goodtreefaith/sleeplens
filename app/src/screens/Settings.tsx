import { useRef, useState } from "react";
import { api } from "../api";
import type { Go } from "../App";
import { Icon } from "../components/Icon";
import { useData, errorText } from "../lib/data";
import { bytes, shortDateY, unitLabel } from "../lib/format";
import type { ThemePref } from "../lib/theme";

export function SettingsScreen({ go, themePref, setThemePref }: { go: Go; themePref: ThemePref; setThemePref: (p: ThemePref) => void }) {
  const { info, refresh } = useData();
  const dlg = useRef<HTMLDialogElement>(null);
  const doneRef = useRef<HTMLParagraphElement>(null);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [done, setDone] = useState(false);
  const d = info?.device;

  async function del() {
    setBusy(true);
    setErr(null);
    try {
      await api.deleteAllData();
      dlg.current?.close();
      setDone(true);
      await refresh();
      // The delete button is now disabled: move focus to the confirmation so keyboard users are not lost.
      setTimeout(() => doneRef.current?.focus(), 0);
    } catch (e) {
      setErr(errorText(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section aria-labelledby="h-settings">
      <div className="screen-h"><h1 id="h-settings">Settings</h1></div>
      <div className="settings-grid">
        <div className="stack">
          <section className="card" aria-labelledby="s-data">
            <div className="card-h"><h2 id="s-data">Your data</h2></div>
            <p className="quiet">Everything SleepLens copied from your card is kept in this folder on this computer. Nothing is sent anywhere.</p>
            <code className="path">{info?.dataDir ?? "–"}</code>
            <dl className="kv" style={{ marginTop: 14 }}>
              <dt>Database size</dt><dd>{info ? bytes(info.dbSizeBytes) : "–"}</dd>
              <dt>Nights stored</dt><dd>{info?.nightCount ?? 0}</dd>
              <dt>First night</dt><dd>{info?.firstDate ? shortDateY(info.firstDate) : "–"}</dd>
              <dt>Last night</dt><dd>{info?.lastDate ? shortDateY(info.lastDate) : "–"}</dd>
              <dt>Last import</dt><dd>{info?.lastImportAt ? `${shortDateY(info.lastImportAt.slice(0, 10))}, ${info.lastImportAt.slice(11, 16)}` : "–"}</dd>
            </dl>
            <div style={{ display: "flex", flexWrap: "wrap", gap: 10, marginTop: 16 }}>
              <button className="btn" type="button" onClick={() => go({ screen: "import" })}><Icon name="card" />Import a card</button>
            </div>
          </section>
          <section className="card" aria-labelledby="s-del">
            <div className="card-h"><h2 id="s-del">Delete all data</h2></div>
            <p className="quiet">Removes every night, setting and the database from this computer. Your SD card is not touched, so you can import it again later.</p>
            {done ? <p className="banner info" role="status" tabIndex={-1} ref={doneRef} style={{ marginTop: 12 }}><Icon name="good" /><span>All data was deleted from this computer.</span></p> : null}
            <div style={{ marginTop: 16 }}>
              <button className="btn danger" type="button" disabled={!info || info.nightCount === 0} onClick={() => { setErr(null); dlg.current?.showModal(); }}><Icon name="trash" />Delete all data…</button>
            </div>
          </section>
        </div>
        <div className="stack">
          <section className="card" aria-labelledby="s-dev">
            <div className="card-h"><h2 id="s-dev">Machine</h2></div>
            {d ? (
              <dl className="kv">
                <dt>Brand</dt><dd>{d.brand}</dd>
                <dt>Model</dt><dd>{d.model}</dd>
                <dt>Card format</dt><dd>{d.family}</dd>
                <dt>Serial number</dt><dd>{d.serialMasked}</dd>
                <dt>Firmware</dt><dd>{d.firmware ?? "–"}</dd>
                <dt>Hardware</dt><dd>{d.hardware ?? "–"}</dd>
                <dt>Pressure unit</dt><dd>{unitLabel(d.pressureUnit) || "unknown"}</dd>
              </dl>
            ) : <p className="quiet">No card imported yet.</p>}
            {d ? <p className="quiet" style={{ marginTop: 12 }}>Only the last two characters of the serial number are shown. Pressures are shown in the unit your machine uses.</p> : null}
          </section>
          <section className="card" aria-labelledby="s-theme">
            <div className="card-h"><h2 id="s-theme">Appearance</h2></div>
            <div className="radio-row" role="radiogroup" aria-label="Theme">
              {(["system", "light", "dark"] as ThemePref[]).map((p) => (
                <label key={p}>
                  <input type="radio" name="theme" value={p} checked={themePref === p} onChange={() => setThemePref(p)} />
                  <Icon name={p === "system" ? "monitor" : p === "light" ? "sun" : "moon"} />
                  {p === "system" ? "Match system" : p === "light" ? "Light" : "Dark"}
                </label>
              ))}
            </div>
          </section>
          <section className="card" aria-labelledby="s-about">
            <div className="card-h"><h2 id="s-about">About</h2></div>
            <dl className="kv"><dt>Version</dt><dd>{info?.version ?? "–"}</dd><dt>Network access</dt><dd>None</dd></dl>
            <p className="quiet" style={{ marginTop: 12 }}>SleepLens explains what your machine recorded. It is not medical advice and never suggests changing settings.</p>
          </section>
        </div>
      </div>

      <dialog className="confirm" ref={dlg} aria-labelledby="dlg-h">
        <h2 id="dlg-h">Delete all data from this computer?</h2>
        <p>This removes {info?.nightCount ?? 0} nights and the database at the folder above. It cannot be undone. Your SD card is not changed.</p>
        {err ? <p className="error-box" role="alert" style={{ marginTop: 12 }}>{err}</p> : null}
        <div className="actions">
          <button className="btn" type="button" autoFocus onClick={() => dlg.current?.close()}>Keep my data</button>
          <button className="btn danger" type="button" disabled={busy} onClick={del}>{busy ? "Deleting…" : "Delete all data"}</button>
        </div>
      </dialog>
    </section>
  );
}
