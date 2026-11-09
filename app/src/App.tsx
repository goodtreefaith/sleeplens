import { useCallback, useEffect, useState } from "react";
import { api, isTauri } from "./api";
import { Icon, Logo } from "./components/Icon";
import { useData } from "./lib/data";
import { parseHash, toHash, type Route, type Screen } from "./lib/route";
import { useTheme } from "./lib/theme";
import { ImportScreen, importPercent } from "./screens/Import";
import { LearnScreen } from "./screens/Learn";
import { NightScreen } from "./screens/Night";
import { OverviewScreen } from "./screens/Overview";
import { SettingsScreen } from "./screens/Settings";
import { TrendsScreen } from "./screens/Trends";

const NAV: { screen: Screen; label: string; icon: string; needsData?: boolean }[] = [
  { screen: "overview", label: "Overview", icon: "home", needsData: true },
  { screen: "night", label: "Night", icon: "moon", needsData: true },
  { screen: "trends", label: "Trends", icon: "trend", needsData: true },
  { screen: "import", label: "Import", icon: "card" },
  { screen: "settings", label: "Settings", icon: "gear" },
  { screen: "learn", label: "Learn", icon: "book" }
];

export type Go = (r: Route) => void;
const NATIVE = isTauri();

export function App() {
  const data = useData();
  const [route, setRoute] = useState<Route>(() => parseHash(location.hash));
  const [themePref, setThemePref, theme] = useTheme();

  useEffect(() => {
    if (NATIVE) document.documentElement.classList.add("native");
    // Back / forward: keep the browser's scroll restoration (no scroll to top here).
    const on = () => setRoute(parseHash(location.hash));
    window.addEventListener("hashchange", on);
    window.addEventListener("popstate", on);
    return () => { window.removeEventListener("hashchange", on); window.removeEventListener("popstate", on); };
  }, []);

  const go: Go = useCallback((r) => {
    const h = toHash(r);
    if (location.hash !== h) history.pushState(null, "", h);
    setRoute(r);
    window.scrollTo(0, 0);
  }, []);

  const hasData = (data.info?.nightCount ?? 0) > 0;
  // First launch (or after deleting data): the data screens fall back to Import.
  const screen: Screen = !hasData && !data.loading && (route.screen === "overview" || route.screen === "night" || route.screen === "trends") ? "import" : route.screen;
  const device = data.info?.device;
  const imp = data.imp;

  return (
    <div className="app">
      <nav className="side" aria-label="Main">
        <div className="brand" data-tauri-drag-region><Logo /><b>SleepLens</b></div>
        <div className="nav">
          {NAV.map((n, i) => (
            <span key={n.screen} style={{ display: "contents" }}>
              {i === 3 ? <div className="sep" role="separator" /> : null}
              <button
                type="button"
                aria-current={screen === n.screen ? "page" : undefined}
                disabled={n.needsData && !hasData}
                title={n.label}
                onClick={() => go({ screen: n.screen, date: n.screen === "overview" || n.screen === "night" ? route.date : undefined })}
              >
                <Icon name={n.icon} />
                <span>{n.label}</span>
                {n.screen === "import" && imp.phase === "running" ? <span className="nav-busy" aria-label="Import running" /> : null}
              </button>
            </span>
          ))}
        </div>
        <div className="side-foot" role="note">
          <strong>Not medical advice</strong>
          <span className="long">SleepLens explains what your machine recorded. Ranges are general guides. Talk to your clinician before changing anything.</span>
        </div>
      </nav>

      <div className="main">
        <header className="topbar" data-tauri-drag-region>
          {device ? (
            <span className="device" title="Device on the last imported card">
              <i aria-hidden="true" />{device.brand} {device.model} · serial {device.serialMasked}
            </span>
          ) : (
            <span className="device" title="No card imported yet"><i aria-hidden="true" style={{ background: "var(--neutral)" }} />No card imported yet</span>
          )}
          {imp.phase === "running" ? (
            <button className="import-pill" type="button" onClick={() => go({ screen: "import" })} title="Show import progress">
              <span className="mini-progress" aria-hidden="true"><span style={{ width: `${importPercent(imp.progress)}%` }} /></span>
              Importing… {imp.progress?.phase === "nights" ? `${imp.progress.done} of ${imp.progress.total}` : ""}
            </button>
          ) : null}
          {api.kind === "mock" ? <span className="quiet preview-note">Browser preview with made-up data</span> : null}
          <span className="spacer" data-tauri-drag-region />
        </header>

        <main className="content" id="content">
          {data.error ? <p className="error-box" role="alert">Could not read your data: {data.error}</p> : null}
          {data.loading ? (
            <div className="loading" role="status">Loading…</div>
          ) : screen === "import" ? (
            <ImportScreen go={go} />
          ) : screen === "overview" ? (
            <OverviewScreen date={route.date} go={go} />
          ) : screen === "night" ? (
            <NightScreen date={route.date} go={go} theme={theme} />
          ) : screen === "trends" ? (
            <TrendsScreen go={go} />
          ) : screen === "settings" ? (
            <SettingsScreen go={go} themePref={themePref} setThemePref={setThemePref} />
          ) : (
            <LearnScreen term={route.term} />
          )}
        </main>
      </div>
    </div>
  );
}
