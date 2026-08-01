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
      root.unmount();
    });
  });
});
