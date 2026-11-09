import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "uplot/dist/uPlot.min.css";
import "./styles.css";
import { App } from "./App";
import { DataProvider } from "./lib/data";
import { applyTheme, readThemePref } from "./lib/theme";

applyTheme(readThemePref());

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <DataProvider>
      <App />
    </DataProvider>
  </StrictMode>
);
