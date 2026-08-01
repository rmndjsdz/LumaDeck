import { describe, expect, it } from "vitest";

import { FocusRegistry } from "./focus-registry";

function element(): HTMLElement {
  const node = document.createElement("div");
  document.body.appendChild(node);
  return node;
}

describe("FocusRegistry", () => {
  it("registers, updates, and unregisters entries", () => {
    const registry = new FocusRegistry();
    const node = element();
    const unregister = registry.register({
      focusId: "first",
      scopeId: "demo",
      element: node,
    });

    expect(registry.count()).toBe(1);
    expect(registry.getScopeEntries("demo")).toHaveLength(1);
    registry.update("first", { disabled: true });
    expect(registry.getScopeEntries("demo")).toHaveLength(0);
    unregister();
    expect(registry.count()).toBe(0);
  });

  it("detects duplicate focus ids and ignores disconnected nodes", () => {
    const registry = new FocusRegistry();
    const first = element();
    const second = element();
    registry.register({
      focusId: "duplicate",
      scopeId: "demo",
      element: first,
    });
    expect(() =>
      registry.register({
        focusId: "duplicate",
        scopeId: "demo",
        element: second,
      }),
    ).toThrow("Duplicate focusId");
    first.remove();
    expect(registry.getScopeEntries("demo")).toHaveLength(0);
  });
});
