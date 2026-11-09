import { afterEach, describe, expect, it, vi } from "vitest";
import { applyTheme, readThemePref } from "./theme";

afterEach(() => vi.unstubAllGlobals());

describe("theme preference", () => {
  it("defaults to Match system for new users and invalid saved preferences", () => {
    for (const saved of [null, "system", "invalid"]) {
      vi.stubGlobal("localStorage", { getItem: () => saved });
      expect(readThemePref()).toBe("system");
    }
  });
  it("preserves explicit light and dark choices", () => {
    for (const saved of ["light", "dark"]) {
      vi.stubGlobal("localStorage", { getItem: () => saved });
      expect(readThemePref()).toBe(saved);
    }
  });
  it("clears the manual theme override when Match system is selected", () => {
    const removeAttribute = vi.fn();
    const setAttribute = vi.fn();
    const setItem = vi.fn();
    vi.stubGlobal("document", { documentElement: { removeAttribute, setAttribute } });
    vi.stubGlobal("localStorage", { setItem });
    applyTheme("system");
    expect(removeAttribute).toHaveBeenCalledWith("data-theme");
    expect(setAttribute).not.toHaveBeenCalled();
    expect(setItem).toHaveBeenCalledWith("sleeplens.theme", "system");
    applyTheme("light");
    expect(setAttribute).toHaveBeenCalledWith("data-theme", "light");
    expect(setItem).toHaveBeenCalledWith("sleeplens.theme", "light");
  });
  it("can follow the system when local storage is unavailable", () => {
    vi.stubGlobal("localStorage", { getItem: () => { throw new Error("Unavailable"); } });
    expect(readThemePref()).toBe("system");
  });
});
