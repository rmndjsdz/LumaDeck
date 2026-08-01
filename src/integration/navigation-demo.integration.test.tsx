import { act } from "react";
import { createRoot } from "react-dom/client";
import { describe, expect, it } from "vitest";
import App from "../App";
import { useProductStore } from "../stores/product-store";

async function waitForCatalog(): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

async function waitForSelector(
  host: HTMLElement,
  selector: string,
): Promise<void> {
  for (let attempt = 0; attempt < 30; attempt += 1) {
    if (host.querySelector(selector)) return;
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
  throw new Error(`Selector did not appear: ${selector}`);
}

async function renderProductApp() {
  useProductStore.setState({
    activeView: "home",
    selectedGameId: null,
    returnView: "home",
    returnFocusId: null,
  });
  const host = document.createElement("div");
  document.body.appendChild(host);
  const root = createRoot(host);
  await act(async () => {
    root.render(<App />);
  });
  await act(waitForCatalog);
  return { host, root };
}

describe("LumaDeck product slice integration", () => {
  it("mounts a persistent shell with a stable home focus", async () => {
    const { host, root } = await renderProductApp();

    expect(host.querySelector(".app-shell")).not.toBeNull();
    expect(host.querySelector('[data-focus-id="shell-home"]')).not.toBeNull();
    expect(
      host.querySelectorAll('[data-focusable="true"]').length,
    ).toBeGreaterThan(10);
    expect(host.querySelector('[data-active="true"]')).not.toBeNull();

    await act(async () => root.unmount());
  });

  it("switches to Library without unmounting the shell and opens Details", async () => {
    const { host, root } = await renderProductApp();
    const shell = host.querySelector(".app-shell");
    const libraryButton = host.querySelector<HTMLElement>(
      '[data-focus-id="shell-library"]',
    );
    expect(libraryButton).not.toBeNull();

    await act(async () => useProductStore.getState().setView("library"));
    await act(async () => waitForSelector(host, "#library-heading"));
    expect(host.querySelector(".app-shell")).toBe(shell);
    expect(host.querySelector("#library-heading")).not.toBeNull();
    expect(
      host.querySelectorAll('[data-focus-id^="library-game-"]').length,
    ).toBeLessThanOrEqual(60);

    const firstGame = host.querySelector<HTMLElement>(
      '[data-focus-id="library-game-001"]',
    );
    expect(firstGame).not.toBeNull();
    await act(async () =>
      useProductStore
        .getState()
        .openDetails("game-001", "library", "library-game-001"),
    );
    expect(host.querySelector("#details-heading")).not.toBeNull();
    expect(
      host
        .querySelector('[data-focus-id="details-play"]')
        ?.getAttribute("data-active"),
    ).toBe("true");

    await act(async () => useProductStore.getState().closeDetails());
    expect(host.querySelector("#library-heading")).not.toBeNull();
    expect(
      host
        .querySelector('[data-focus-id="library-game-001"]')
        ?.getAttribute("data-active"),
    ).toBe("true");

    await act(async () => root.unmount());
  });

  it("filters the local library without changing the shell", async () => {
    const { host, root } = await renderProductApp();
    await act(async () => useProductStore.getState().setView("library"));
    await act(async () => waitForSelector(host, "#library-heading"));
    const input = host.querySelector<HTMLInputElement>(
      'input[placeholder="Search games"]',
    );
    expect(input).not.toBeNull();
    if (input) {
      input.focus();
      const setter = Object.getOwnPropertyDescriptor(
        HTMLInputElement.prototype,
        "value",
      )?.set;
      setter?.call(input, "Aether");
      await act(async () => {
        input.dispatchEvent(new Event("input", { bubbles: true }));
        await waitForSelector(host, '[data-focus-id^="library-game-"]');
      });
    }
    expect(
      host.querySelectorAll('[data-focus-id^="library-game-"]').length,
    ).toBeGreaterThan(0);
    expect(
      host.querySelectorAll('[data-focus-id^="library-game-"]').length,
    ).toBeLessThan(200);
    expect(host.querySelector(".app-shell")).not.toBeNull();

    await act(async () => root.unmount());
  });
});
