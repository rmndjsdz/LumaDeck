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

  it("moves linear groups by index without falling back to spatial navigation", () => {
    resetStore();
    const registry = new FocusRegistry();
    const engine = new NavigationEngine(registry, new FocusScrollManager());
    for (const [index, id] of ["tab-0", "tab-1", "tab-2"].entries()) {
      registry.register({
        focusId: id,
        scopeId: "root",
        element: addElement(new DOMRect(index * 100, 0, 80, 40)),
        linearNavigation: {
          groupId: "tabs",
          axis: "horizontal",
          wrap: false,
        },
      });
    }
    engine.registerScope({
      scopeId: "root",
      initialFocusId: "tab-0",
      activateOnMount: true,
    });

    expect(engine.dispatch("move-left")).toBe(false);
    expect(engine.getActiveFocusId()).toBe("tab-0");
    expect(engine.dispatch("move-right")).toBe(true);
    expect(engine.getActiveFocusId()).toBe("tab-1");
    expect(engine.dispatch("move-right")).toBe(true);
    expect(engine.getActiveFocusId()).toBe("tab-2");
    expect(engine.dispatch("move-right")).toBe(false);
    expect(engine.getActiveFocusId()).toBe("tab-2");
  });

  it("wraps linear groups when explicitly enabled", () => {
    resetStore();
    const registry = new FocusRegistry();
    const engine = new NavigationEngine(registry, new FocusScrollManager());
    for (const [index, id] of ["tab-0", "tab-1"].entries()) {
      registry.register({
        focusId: id,
        scopeId: "root",
        element: addElement(new DOMRect(index * 100, 0, 80, 40)),
        linearNavigation: {
          groupId: "tabs",
          axis: "horizontal",
          wrap: true,
        },
      });
    }
    engine.registerScope({
      scopeId: "root",
      initialFocusId: "tab-0",
      activateOnMount: true,
    });

    expect(engine.dispatch("move-left")).toBe(true);
    expect(engine.getActiveFocusId()).toBe("tab-1");
    expect(engine.dispatch("move-right")).toBe(true);
    expect(engine.getActiveFocusId()).toBe("tab-0");
  });

  it("pauses parent scopes and traps modal focus until the modal closes", () => {
    resetStore();
    const registry = new FocusRegistry();
    const scrollManager = new FocusScrollManager();
    const engine = new NavigationEngine(registry, scrollManager);
    const scrollScope = document.createElement("div");
    scrollScope.dataset.scrollScope = "root";
    document.body.appendChild(scrollScope);
    scrollScope.scrollTop = 120;
    scrollScope.scrollLeft = 33;
    registry.register({
      focusId: "open",
      scopeId: "root",
      element: addElement(new DOMRect(0, 0, 80, 40)),
    });
    registry.register({
      focusId: "modal-1",
      scopeId: "modal",
      element: addElement(new DOMRect(0, 0, 80, 40)),
    });
    registry.register({
      focusId: "modal-2",
      scopeId: "modal",
      element: addElement(new DOMRect(100, 0, 80, 40)),
    });
    engine.registerScope({
      scopeId: "root",
      initialFocusId: "open",
      activateOnMount: true,
    });
    engine.prepareScopeOpen("modal", "open");
    engine.registerScope({
      scopeId: "modal",
      initialFocusId: "modal-1",
      trapFocus: true,
      modal: true,
      restoreFocus: true,
    });

    expect(engine.focus("open")).toBe(false);
    expect(engine.getActiveScopeId()).toBe("modal");
    expect(engine.dispatch("move-right")).toBe(true);
    expect(engine.getActiveFocusId()).toBe("modal-2");
    expect(engine.handleTab(false)).toBe(true);
    expect(engine.getActiveFocusId()).toBe("modal-1");
    expect(engine.handleTab(true)).toBe(true);
    expect(engine.getActiveFocusId()).toBe("modal-2");

    scrollScope.scrollTop = 0;
    scrollScope.scrollLeft = 0;
    engine.unregisterScope("modal");
    expect(engine.getActiveScopeId()).toBe("root");
    expect(engine.getActiveFocusId()).toBe("open");
    expect(scrollScope.scrollTop).toBe(120);
    expect(scrollScope.scrollLeft).toBe(33);
  });

  it("re-activates a modal after a strict-mode mount cycle", () => {
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
      modal: true,
      activateOnMount: true,
    });
    engine.unregisterScope("modal");
    engine.registerScope({
      scopeId: "modal",
      initialFocusId: "modal-action",
      modal: true,
      activateOnMount: true,
    });

    expect(engine.getActiveScopeId()).toBe("modal");
    expect(engine.getActiveFocusId()).toBe("modal-action");
  });
});
