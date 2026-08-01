import { act } from "react";
import { createRoot } from "react-dom/client";
import { describe, expect, it } from "vitest";

import App from "../App";

describe("navigation demo integration", () => {
  it("mounts with registered focusables and an initial active focus", async () => {
    const host = document.createElement("div");
    document.body.appendChild(host);
    const root = createRoot(host);

    await act(async () => {
      root.render(<App />);
    });

    expect(
      host.querySelectorAll('[data-focusable="true"]').length,
    ).toBeGreaterThan(10);
    expect(host.querySelector('[data-active="true"]')).not.toBeNull();
    expect(document.documentElement.dataset.inputMode).toBe("mouse");

    await act(async () => {
      window.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "ArrowRight",
          bubbles: true,
          cancelable: true,
        }),
      );
    });
    expect(
      host
        .querySelector('[data-focus-id="tab-library"]')
        ?.getAttribute("data-active"),
    ).toBe("true");

    await act(async () => {
      root.unmount();
    });
  });

  it("keeps a modal exclusive and restores its opener", async () => {
    const host = document.createElement("div");
    document.body.appendChild(host);
    const root = createRoot(host);

    await act(async () => {
      root.render(<App />);
    });
    const openButton = host.querySelector<HTMLElement>(
      '[data-focus-id="overview-open-modal"]',
    );
    expect(openButton).not.toBeNull();

    await act(async () => {
      openButton?.click();
    });
    expect(
      host.querySelector('[data-navigation-dialog="true"]'),
    ).not.toBeNull();
    expect(
      host
        .querySelector('[data-focus-id="modal-primary"]')
        ?.getAttribute("data-active"),
    ).toBe("true");

    await act(async () => {
      window.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "Tab",
          bubbles: true,
          cancelable: true,
        }),
      );
    });
    expect(
      host
        .querySelector('[data-focus-id="modal-secondary"]')
        ?.getAttribute("data-active"),
    ).toBe("true");

    await act(async () => {
      host.querySelector<HTMLElement>('[data-focus-id="tab-library"]')?.click();
    });
    expect(
      host
        .querySelector('[data-focus-id="modal-secondary"]')
        ?.getAttribute("data-active"),
    ).toBe("true");

    await new Promise((resolve) => setTimeout(resolve, 60));
    await act(async () => {
      host.querySelector<HTMLElement>('[data-focus-id="modal-close"]')?.click();
    });
    expect(host.querySelector('[data-navigation-dialog="true"]')).toBeNull();
    expect(
      host
        .querySelector('[data-focus-id="overview-open-modal"]')
        ?.getAttribute("data-active"),
    ).toBe("true");

    await act(async () => {
      root.unmount();
    });
  });
});
