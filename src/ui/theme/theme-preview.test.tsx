import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { useThemeStore } from "../../stores/theme-store";
import { DEFAULT_THEME_ID } from "./theme-registry";
import { ThemeProvider } from "./ThemeProvider";
import { useTheme } from "./theme-context";

function Harness() {
  const {
    theme,
    confirmedTheme,
    previewThemeId,
    previewTheme,
    clearThemePreview,
    setTheme,
  } = useTheme();
  return (
    <div>
      <span data-testid="theme">{theme.id}</span>
      <span data-testid="confirmed">{confirmedTheme.id}</span>
      <span data-testid="preview">{previewThemeId ?? "none"}</span>
      <button onClick={() => previewTheme("lumadeck-cinematic")}>
        preview
      </button>
      <button onClick={clearThemePreview}>clear</button>
      <button onClick={() => setTheme("lumadeck-cinematic")}>confirm</button>
    </div>
  );
}

describe("theme preview runtime", () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    window.localStorage.clear();
    useThemeStore.setState({ activeThemeId: DEFAULT_THEME_ID });
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    act(() => {
      root.render(
        <ThemeProvider>
          <Harness />
        </ThemeProvider>,
      );
    });
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
    window.localStorage.clear();
    useThemeStore.setState({ activeThemeId: DEFAULT_THEME_ID });
  });

  it("previews without changing confirmed state or persistence", () => {
    act(() => {
      host.querySelector<HTMLButtonElement>("button")?.click();
    });

    expect(host.querySelector('[data-testid="theme"]')?.textContent).toBe(
      "lumadeck-cinematic",
    );
    expect(host.querySelector('[data-testid="confirmed"]')?.textContent).toBe(
      DEFAULT_THEME_ID,
    );
    expect(window.localStorage.getItem("lumadeck.active-theme")).toBeNull();
  });

  it("clears a preview with B semantics and persists only on confirm", () => {
    act(() => {
      host.querySelector<HTMLButtonElement>("button")?.click();
      host.querySelectorAll<HTMLButtonElement>("button")[1]?.click();
    });
    expect(host.querySelector('[data-testid="preview"]')?.textContent).toBe(
      "none",
    );

    act(() => {
      host.querySelectorAll<HTMLButtonElement>("button")[0]?.click();
      host.querySelectorAll<HTMLButtonElement>("button")[2]?.click();
    });
    expect(host.querySelector('[data-testid="preview"]')?.textContent).toBe(
      "none",
    );
    expect(host.querySelector('[data-testid="confirmed"]')?.textContent).toBe(
      "lumadeck-cinematic",
    );
    expect(window.localStorage.getItem("lumadeck.active-theme")).toBe(
      "lumadeck-cinematic",
    );
  });
});
