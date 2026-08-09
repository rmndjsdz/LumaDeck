import { describe, expect, it, vi } from "vitest";
import { FocusScrollManager } from "./focus-scroll-manager";

function createNestedFocusable(childRect: DOMRect, parentRect: DOMRect) {
  const scroller = document.createElement("div");
  scroller.style.overflowY = "auto";
  Object.defineProperty(scroller, "scrollHeight", { value: 500 });
  Object.defineProperty(scroller, "clientHeight", { value: 200 });
  vi.spyOn(scroller, "getBoundingClientRect").mockReturnValue(parentRect);

  const child = document.createElement("button");
  vi.spyOn(child, "getBoundingClientRect").mockReturnValue(childRect);
  child.scrollIntoView = vi.fn();
  scroller.append(child);
  document.body.append(scroller);
  return { child, scroller };
}

describe("FocusScrollManager", () => {
  it("scrolls a focused element clipped by a nested scroll container", () => {
    const { child } = createNestedFocusable(
      new DOMRect(100, 280, 100, 50),
      new DOMRect(0, 100, 300, 200),
    );

    const result = new FocusScrollManager().ensureVisible(
      child,
      "network-wifi-6",
    );

    expect(result.scrolled).toBe(true);
    expect(child.scrollIntoView).toHaveBeenCalledWith(
      expect.objectContaining({ block: "nearest", inline: "nearest" }),
    );
  });

  it("does not scroll a focused element visible in its nested container", () => {
    const { child } = createNestedFocusable(
      new DOMRect(100, 140, 100, 50),
      new DOMRect(0, 100, 300, 200),
    );

    const result = new FocusScrollManager().ensureVisible(
      child,
      "network-wifi-1",
    );

    expect(result.scrolled).toBe(false);
    expect(child.scrollIntoView).not.toHaveBeenCalled();
  });
});
