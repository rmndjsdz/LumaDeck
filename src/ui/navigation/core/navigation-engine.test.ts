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
    inputMode: "mouse",
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

  it("waits for a delayed scope focusable and blocks the parent while waiting", () => {
    resetStore();
    const registry = new FocusRegistry();
    const engine = new NavigationEngine(registry, new FocusScrollManager());
    registry.register({
      focusId: "library-game-0",
      scopeId: "library",
      element: addElement(new DOMRect()),
    });
    engine.registerScope({
      scopeId: "library",
      initialFocusId: "library-game-0",
      activateOnMount: true,
    });
    engine.prepareScopeOpen("details", "library-game-0");
    engine.registerScope({
      scopeId: "details",
      parentScopeId: "library",
      initialFocusId: "details-play",
      activateOnMount: true,
      modal: true,
    });

    expect(engine.getActiveScopeId()).toBe("library");
    expect(engine.getScopeLifecycleState("details")).toBe(
      "waiting-for-focusable",
    );
    expect(engine.dispatch("move-right")).toBe(false);

    registry.register({
      focusId: "details-play",
      scopeId: "details",
      element: addElement(new DOMRect()),
    });

    expect(engine.getActiveScopeId()).toBe("details");
    expect(engine.getActiveFocusId()).toBe("details-play");
    expect(engine.getScopeLifecycleState("details")).toBe("active");
  });

  it("falls back to the first valid focusable when the initial focus is missing or disabled", () => {
    resetStore();
    const registry = new FocusRegistry();
    const engine = new NavigationEngine(registry, new FocusScrollManager());
    registry.register({
      focusId: "disabled-action",
      scopeId: "details",
      element: addElement(new DOMRect()),
      disabled: true,
    });
    registry.register({
      focusId: "details-back",
      scopeId: "details",
      element: addElement(new DOMRect()),
    });
    engine.registerScope({
      scopeId: "details",
      initialFocusId: "details-play",
      activateOnMount: true,
    });

    expect(engine.getActiveScopeId()).toBe("details");
    expect(engine.getActiveFocusId()).toBe("details-back");
  });

  it("keeps logical and DOM focus synchronized for gamepad input", () => {
    resetStore();
    const registry = new FocusRegistry();
    const engine = new NavigationEngine(registry, new FocusScrollManager());
    const button = document.createElement("button");
    document.body.appendChild(button);
    registry.register({
      focusId: "details-play",
      scopeId: "details",
      element: button,
    });
    useNavigationStore.getState().setInputMode("gamepad");
    engine.registerScope({
      scopeId: "details",
      initialFocusId: "details-play",
      activateOnMount: true,
    });

    expect(engine.getActiveFocusId()).toBe("details-play");
    expect(document.activeElement).toBe(button);
    expect(useNavigationStore.getState().debug.activeFocusValid).toBe(true);
  });

  it("keeps grid navigation in the current row and column", () => {
    resetStore();
    const registry = new FocusRegistry();
    const engine = new NavigationEngine(registry, new FocusScrollManager());
    for (let index = 0; index < 10; index += 1) {
      registry.register({
        focusId: `cell-${index}`,
        scopeId: "root",
        element: addElement(
          new DOMRect((index % 5) * 100, Math.floor(index / 5) * 60, 80, 40),
        ),
        disabled: index === 0,
        gridNavigation: { groupId: "grid", columns: 5 },
      });
    }
    engine.registerScope({
      scopeId: "root",
      initialFocusId: "cell-1",
      activateOnMount: true,
    });

    expect(engine.dispatch("move-left")).toBe(false);
    expect(engine.getActiveFocusId()).toBe("cell-1");
    expect(engine.dispatch("move-right")).toBe(true);
    expect(engine.getActiveFocusId()).toBe("cell-2");
    expect(engine.dispatch("move-down")).toBe(true);
    expect(engine.getActiveFocusId()).toBe("cell-7");
    expect(engine.dispatch("move-up")).toBe(true);
    expect(engine.getActiveFocusId()).toBe("cell-2");
  });

  it("requests an off-window grid index instead of falling back spatially", () => {
    resetStore();
    const registry = new FocusRegistry();
    const engine = new NavigationEngine(registry, new FocusScrollManager());
    const requested: number[] = [];
    for (let index = 0; index < 5; index += 1) {
      registry.register({
        focusId: `cell-${index}`,
        scopeId: "root",
        element: addElement(new DOMRect((index % 5) * 100, 0, 80, 40)),
        gridNavigation: {
          groupId: "grid",
          columns: 5,
          index,
          itemCount: 10,
          resolveFocusId: (targetIndex) => `cell-${targetIndex}`,
          onRequestIndex: (targetIndex) => requested.push(targetIndex),
        },
      });
    }
    engine.registerScope({
      scopeId: "root",
      initialFocusId: "cell-0",
      activateOnMount: true,
    });

    expect(engine.dispatch("move-down")).toBe(true);
    expect(requested).toEqual([5]);
    expect(engine.getActiveFocusId()).toBe("cell-0");
  });

  it("reproduces focus loss while a non-first-column target is rematerialized", async () => {
    resetStore();
    const registry = new FocusRegistry();
    const engine = new NavigationEngine(registry, new FocusScrollManager());
    const registered = new Map<number, () => void>();
    const requestedTargets: number[] = [];
    let windowStart = 0;

    const materialize = (start: number) => {
      for (const unregister of registered.values()) unregister();
      registered.clear();
      windowStart = start;
      for (let index = start; index < Math.min(start + 60, 200); index += 1) {
        const node = addElement(
          new DOMRect((index % 5) * 100, Math.floor(index / 5) * 60, 80, 40),
        );
        const unregister = registry.register({
          focusId: `cell-${index}`,
          scopeId: "root",
          element: node,
          gridNavigation: {
            groupId: "grid",
            columns: 5,
            index,
            itemCount: 200,
            resolveFocusId: (targetIndex) => `cell-${targetIndex}`,
            onRequestIndex: (targetIndex) => {
              requestedTargets.push(targetIndex);
              materialize(Math.max(0, Math.min(targetIndex - 55, 140)));
            },
          },
        });
        registered.set(index, unregister);
      }
    };

    materialize(0);
    engine.registerScope({
      scopeId: "root",
      initialFocusId: "cell-2",
      activateOnMount: true,
    });

    for (let step = 0; step < 11; step += 1) {
      expect(engine.dispatch("move-down")).toBe(true);
    }
    expect(engine.getActiveFocusId()).toBe("cell-57");
    expect(engine.dispatch("move-down")).toBe(true);
    expect(windowStart).toBe(7);
    expect(engine.dispatch("move-down")).toBe(true);
    expect(requestedTargets).toEqual([62, 67]);

    await new Promise<void>((resolve) => {
      window.requestAnimationFrame(() => resolve());
    });

    expect(engine.getActiveFocusId()).toBe("cell-67");
    expect(engine.dispatch("move-right")).toBe(true);
    expect(engine.getActiveFocusId()).toBe("cell-68");
  });

  it("reproduces Home row gaps when vertical navigation relies on geometry", () => {
    resetStore();
    const registry = new FocusRegistry();
    const engine = new NavigationEngine(registry, new FocusScrollManager());
    const rowIds = ["home-row-0", "home-row-1", "home-row-2"];

    for (let rowIndex = 0; rowIndex < rowIds.length; rowIndex += 1) {
      for (let itemIndex = 0; itemIndex < 5; itemIndex += 1) {
        const visualColumn = rowIndex === 1 ? 4 - itemIndex : itemIndex;
        registry.register({
          focusId: `${rowIds[rowIndex]}-${itemIndex}`,
          scopeId: "home",
          element: addElement(
            new DOMRect(visualColumn * 100, rowIndex * 100, 80, 40),
          ),
          linearNavigation: {
            groupId: rowIds[rowIndex],
            axis: "horizontal",
            wrap: false,
          },
          rowNavigation: {
            groupId: "home-rows",
            rowId: rowIds[rowIndex],
            rowIndex,
            itemIndex,
            preserveHorizontalIntent: true,
          },
        });
      }
    }

    engine.registerScope({
      scopeId: "home",
      initialFocusId: "home-row-0-0",
      activateOnMount: true,
    });

    for (let itemIndex = 0; itemIndex < 5; itemIndex += 1) {
      expect(engine.focus(`home-row-0-${itemIndex}`)).toBe(true);
      expect(engine.dispatch("move-down")).toBe(true);
      expect(engine.getActiveFocusId()).toBe(`home-row-1-${itemIndex}`);
    }
  });

  it("resolves every Home column in both vertical directions", () => {
    resetStore();
    const registry = new FocusRegistry();
    const engine = new NavigationEngine(registry, new FocusScrollManager());
    for (let rowIndex = 0; rowIndex < 3; rowIndex += 1) {
      for (let itemIndex = 0; itemIndex < 5; itemIndex += 1) {
        registry.register({
          focusId: `home-${rowIndex}-${itemIndex}`,
          scopeId: "home",
          element: addElement(
            new DOMRect(itemIndex * 100, rowIndex * 100, 80, 40),
          ),
          rowNavigation: {
            groupId: "home-rows",
            rowId: `home-row-${rowIndex}`,
            rowIndex,
            itemIndex,
            preserveHorizontalIntent: true,
          },
          linearNavigation: {
            groupId: `home-row-${rowIndex}`,
            axis: "horizontal",
          },
        });
      }
    }
    engine.registerScope({
      scopeId: "home",
      initialFocusId: "home-0-0",
      activateOnMount: true,
    });

    for (let itemIndex = 0; itemIndex < 5; itemIndex += 1) {
      expect(engine.focus(`home-0-${itemIndex}`)).toBe(true);
      expect(engine.dispatch("move-down")).toBe(true);
      expect(engine.getActiveFocusId()).toBe(`home-1-${itemIndex}`);
      expect(engine.dispatch("move-down")).toBe(true);
      expect(engine.getActiveFocusId()).toBe(`home-2-${itemIndex}`);
      expect(engine.dispatch("move-up")).toBe(true);
      expect(engine.getActiveFocusId()).toBe(`home-1-${itemIndex}`);
      expect(engine.dispatch("move-up")).toBe(true);
      expect(engine.getActiveFocusId()).toBe(`home-0-${itemIndex}`);
    }
  });

  it("keeps preferred horizontal intent across short rows", () => {
    resetStore();
    const registry = new FocusRegistry();
    const engine = new NavigationEngine(registry, new FocusScrollManager());
    const lengths = [5, 5, 4];

    lengths.forEach((length, rowIndex) => {
      for (let itemIndex = 0; itemIndex < length; itemIndex += 1) {
        registry.register({
          focusId: `row-${rowIndex}-${itemIndex}`,
          scopeId: "home",
          element: addElement(
            new DOMRect(itemIndex * 100, rowIndex * 100, 80, 40),
          ),
          rowNavigation: {
            groupId: "home-rows",
            rowId: `row-${rowIndex}`,
            rowIndex,
            itemIndex,
            preserveHorizontalIntent: true,
          },
          linearNavigation: {
            groupId: `row-${rowIndex}`,
            axis: "horizontal",
          },
        });
      }
    });
    engine.registerScope({
      scopeId: "home",
      initialFocusId: "row-0-4",
      activateOnMount: true,
    });

    expect(engine.dispatch("move-down")).toBe(true);
    expect(engine.getActiveFocusId()).toBe("row-1-4");
    expect(engine.dispatch("move-down")).toBe(true);
    expect(engine.getActiveFocusId()).toBe("row-2-3");
    expect(engine.dispatch("move-up")).toBe(true);
    expect(engine.getActiveFocusId()).toBe("row-1-4");
    expect(engine.dispatch("move-up")).toBe(true);
    expect(engine.getActiveFocusId()).toBe("row-0-4");
  });

  it("restores Home row state from Details and navigates vertically immediately", () => {
    resetStore();
    const registry = new FocusRegistry();
    const engine = new NavigationEngine(registry, new FocusScrollManager());
    for (let rowIndex = 0; rowIndex < 3; rowIndex += 1) {
      for (let itemIndex = 0; itemIndex < 5; itemIndex += 1) {
        registry.register({
          focusId: `home-${rowIndex}-${itemIndex}`,
          scopeId: "home",
          element: addElement(
            new DOMRect(itemIndex * 100, rowIndex * 100, 80, 40),
          ),
          rowNavigation: {
            groupId: "home-rows",
            rowId: `home-row-${rowIndex}`,
            rowIndex,
            itemIndex,
            preserveHorizontalIntent: true,
          },
        });
      }
    }
    registry.register({
      focusId: "details-play",
      scopeId: "details",
      element: addElement(new DOMRect()),
    });
    engine.registerScope({
      scopeId: "home",
      initialFocusId: "home-0-0",
      activateOnMount: true,
    });

    for (const rowIndex of [0, 1, 2]) {
      const opener = `home-${rowIndex}-2`;
      expect(engine.focus(opener)).toBe(true);
      engine.prepareScopeOpen("details", opener);
      engine.registerScope({
        scopeId: "details",
        parentScopeId: "home",
        initialFocusId: "details-play",
        modal: true,
        activateOnMount: true,
        restoreFocus: true,
      });
      expect(engine.getActiveFocusId()).toBe("details-play");
      engine.unregisterScope("details");
      expect(engine.getActiveFocusId()).toBe(opener);
      if (rowIndex < 2) {
        expect(engine.dispatch("move-down")).toBe(true);
        expect(engine.getActiveFocusId()).toBe(`home-${rowIndex + 1}-2`);
      }
    }
  });
});
