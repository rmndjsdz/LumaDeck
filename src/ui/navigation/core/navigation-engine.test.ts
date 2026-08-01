import { describe, expect, it } from "vitest";

import { useNavigationStore } from "../../../stores/navigation-store";
import { FocusRegistry } from "./focus-registry";
import { NavigationEngine } from "./navigation-engine";
import type { Rect } from "./navigation-types";
import { FocusScrollManager } from "../scroll/focus-scroll-manager";

function addElement(rect: Rect): HTMLElement {
  const node = document.createElement("div");
  document.body.appendChild(node);
  node.getBoundingClientRect = () =>
    new DOMRect(rect.left, rect.top, rect.width, rect.height);
  return node;
}

function resetStore(): void {
  useNavigationStore.setState({
    activeScopeId: null,
    activeFocusId: null,
    previousFocusId: null,
    lastNavigationAction: null,
  });
}

describe("NavigationEngine", () => {
  it("uses overrides, spatial navigation, disabled omission, and confirmation", () => {
    resetStore();
    const registry = new FocusRegistry();
    const engine = new NavigationEngine(registry, new FocusScrollManager());
    const confirmed: string[] = [];
    const entries = [
      { id: "one", rect: new DOMRect(0, 0, 80, 40) },
      { id: "two", rect: new DOMRect(100, 0, 80, 40) },
      { id: "disabled", rect: new DOMRect(200, 0, 80, 40) },
    ];
    for (const entry of entries) {
      registry.register({
        focusId: entry.id,
        scopeId: "root",
        element: addElement(entry.rect),
        disabled: entry.id === "disabled",
        onConfirm: () => confirmed.push(entry.id),
      });
    }
    engine.registerScope({
      scopeId: "root",
      initialFocusId: "one",
      activateOnMount: true,
    });

    expect(engine.getActiveFocusId()).toBe("one");
    expect(engine.dispatch("move-right")).toBe(true);
    expect(engine.getActiveFocusId()).toBe("two");
    expect(engine.dispatch("confirm")).toBe(true);
    expect(confirmed).toEqual(["two"]);
    expect(engine.dispatch("move-right")).toBe(false);
  });

  it("restores the opener after a modal scope closes", () => {
    resetStore();
    const registry = new FocusRegistry();
    const engine = new NavigationEngine(registry, new FocusScrollManager());
    registry.register({
      focusId: "open",
      scopeId: "root",
      element: addElement(new DOMRect()),
    });
    registry.register({
      focusId: "modal-action",
      scopeId: "modal",
      element: addElement(new DOMRect()),
    });
    engine.registerScope({
      scopeId: "root",
      initialFocusId: "open",
      activateOnMount: true,
    });
    engine.prepareScopeOpen("modal", "open");
    engine.registerScope({
      scopeId: "modal",
      initialFocusId: "modal-action",
      restoreFocus: true,
    });

    expect(engine.getActiveScopeId()).toBe("modal");
    engine.unregisterScope("modal");
    expect(engine.getActiveScopeId()).toBe("root");
    expect(engine.getActiveFocusId()).toBe("open");
  });
});
