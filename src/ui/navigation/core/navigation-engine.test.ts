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
  it("protects primary navigation through generic modal and trapping scope state", () => {
    resetStore();
    const registry = new FocusRegistry();
    const engine = new NavigationEngine(registry, new FocusScrollManager());
    registry.register({
      focusId: "root-focus",
      scopeId: "root",
      element: addElement(new DOMRect(0, 0, 80, 40)),
    });
    registry.register({
      focusId: "dialog-focus",
      scopeId: "dialog",
      element: addElement(new DOMRect(0, 0, 80, 40)),
    });

    engine.registerScope({
      scopeId: "root",
      initialFocusId: "root-focus",
      activateOnMount: true,
    });
    expect(engine.getPrimaryNavigationBlockReason()).toBeNull();

    engine.registerScope({
      scopeId: "dialog",
      parentScopeId: "root",
      initialFocusId: "dialog-focus",
      modal: true,
      trapFocus: true,
      activateOnMount: true,
    });
    expect(engine.getPrimaryNavigationBlockReason()).toBe("modal");
    engine.dispose();
  });

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

  it("records the physical source for directional and pointer selection", () => {
    resetStore();
    const registry = new FocusRegistry();
    const engine = new NavigationEngine(registry, new FocusScrollManager());
    registry.register({
      focusId: "one",
      scopeId: "root",
      element: addElement(new DOMRect(0, 0, 80, 40)),
    });
    registry.register({
      focusId: "two",
      scopeId: "root",
      element: addElement(new DOMRect(100, 0, 80, 40)),
    });
    engine.registerScope({
      scopeId: "root",
      initialFocusId: "one",
      activateOnMount: true,
    });

    expect(engine.dispatch("move-right", "gamepad")).toBe(true);
    expect(engine.focusFromPointer("one")).toBe(true);

    expect(
      engine
        .getNavigationTrace()
        .find(
          (record) =>
            record.event === "NAV_INPUT" && record.direction === "right",
        ),
    ).toMatchObject({ inputSource: "gamepad" });
    expect(
      engine
        .getNavigationTrace()
        .find((record) => record.event === "POINTER_SELECTION"),
    ).toMatchObject({
      inputSource: "mouse",
      focusReason: "pointer-selection",
      selectedFocusId: "one",
    });
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
    expect(engine.requestScopeRestore("modal", "root", "modal-to-root-1")).toBe(
      true,
    );
    engine.unregisterScope("modal");
    expect(engine.getActiveScopeId()).toBe("root");
    expect(engine.getActiveFocusId()).toBe("open");
    expect(
      engine
        .getNavigationTrace()
        .filter((record) => record.event === "CONTEXT_RESTORE_COMMIT"),
    ).toHaveLength(1);
  });

  it("activates an explicitly nested modal scope from a modal parent", () => {
    resetStore();
    const registry = new FocusRegistry();
    const engine = new NavigationEngine(registry, new FocusScrollManager());
    registry.register({
      focusId: "details-open-artwork",
      scopeId: "details",
      element: addElement(new DOMRect()),
    });
    registry.register({
      focusId: "artwork-slot-grid_horizontal",
      scopeId: "artwork-modifier",
      element: addElement(new DOMRect()),
    });
    engine.registerScope({
      scopeId: "details",
      initialFocusId: "details-open-artwork",
      modal: true,
      trapFocus: true,
      activateOnMount: true,
    });

    engine.prepareScopeOpen("artwork-modifier", "details-open-artwork");
    engine.registerScope({
      scopeId: "artwork-modifier",
      parentScopeId: "details",
      initialFocusId: "artwork-slot-grid_horizontal",
      modal: true,
      trapFocus: true,
      activateOnMount: true,
    });

    expect(engine.getActiveScopeId()).toBe("artwork-modifier");
    expect(engine.getActiveFocusId()).toBe("artwork-slot-grid_horizontal");
  });

  it("uses an explicit fallback when a nested modal opener unmounts", () => {
    resetStore();
    const registry = new FocusRegistry();
    const engine = new NavigationEngine(registry, new FocusScrollManager());
    for (const [focusId, scopeId] of [
      ["details-play", "details"],
      ["details-back", "details"],
      ["details-menu-opener", "details"],
      ["artwork-slot-grid_horizontal", "artwork-modifier"],
    ] as const) {
      registry.register({
        focusId,
        scopeId,
        element: addElement(new DOMRect()),
      });
    }
    engine.registerScope({
      scopeId: "details",
      initialFocusId: "details-play",
      modal: true,
      trapFocus: true,
      activateOnMount: true,
    });
    engine.focus("details-menu-opener");
    engine.prepareScopeOpen("artwork-modifier", "details-menu-opener");
    engine.registerScope({
      scopeId: "artwork-modifier",
      parentScopeId: "details",
      initialFocusId: "artwork-slot-grid_horizontal",
      modal: true,
      trapFocus: true,
      activateOnMount: true,
    });

    expect(engine.getActiveFocusId()).toBe("artwork-slot-grid_horizontal");
    expect(
      engine.requestScopeRestore(
        "artwork-modifier",
        "details",
        "artwork-close-fallback",
      ),
    ).toBe(false);
    expect(engine.completePendingRestore("details", "details-back")).toBe(
      false,
    );
    registry.unregister("details-menu-opener");
    engine.unregisterScope("artwork-modifier");

    expect(engine.getActiveScopeId()).toBe("details");
    expect(engine.getActiveFocusId()).toBe("details-back");
  });

  it("falls back to the parent scope when a close restore request races unmount", () => {
    resetStore();
    const registry = new FocusRegistry();
    const engine = new NavigationEngine(registry, new FocusScrollManager());
    registry.register({
      focusId: "parent-focus",
      scopeId: "parent",
      element: addElement(new DOMRect()),
    });
    registry.register({
      focusId: "modal-focus",
      scopeId: "modal",
      element: addElement(new DOMRect()),
    });
    engine.registerScope({
      scopeId: "parent",
      initialFocusId: "parent-focus",
    });
    engine.registerScope({
      scopeId: "modal",
      parentScopeId: "parent",
      initialFocusId: "modal-focus",
      modal: true,
      activateOnMount: true,
    });

    expect(engine.getActiveScopeId()).toBe("modal");
    expect(engine.requestScopeRestore("modal", "parent", "racing-close")).toBe(
      false,
    );

    engine.unregisterScope("modal");

    expect(engine.getActiveScopeId()).toBe("parent");
    expect(engine.getActiveFocusId()).toBe("parent-focus");
  });

  it("keeps one restore transaction across duplicate requests and rematerialization", () => {
    resetStore();
    const registry = new FocusRegistry();
    const engine = new NavigationEngine(registry, new FocusScrollManager());
    const openElement = addElement(new DOMRect());
    const unregisterOpen = registry.register({
      focusId: "open",
      scopeId: "root",
      element: openElement,
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
      restoreFocus: true,
    });
    unregisterOpen();

    expect(engine.requestScopeRestore("modal", "root", "transition-1")).toBe(
      false,
    );
    expect(engine.requestScopeRestore("modal", "root", "transition-1")).toBe(
      false,
    );
    engine.unregisterScope("modal");
    registry.register({
      focusId: "open",
      scopeId: "root",
      element: openElement,
    });

    expect(engine.getActiveScopeId()).toBe("root");
    expect(engine.getActiveFocusId()).toBe("open");
    expect(
      engine
        .getNavigationTrace()
        .filter((record) => record.event === "CONTEXT_RESTORE_COMMIT"),
    ).toHaveLength(1);
    expect(
      engine
        .getNavigationTrace()
        .filter((record) => record.event === "CONTEXT_RESTORE_REQUEST_REUSED"),
    ).toHaveLength(1);
  });

  it("cancels a pending restore only for a different transition", () => {
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
      restoreFocus: true,
    });

    expect(engine.requestScopeRestore("modal", "root", "transition-old")).toBe(
      false,
    );
    expect(engine.requestScopeRestore("modal", "root", "transition-new")).toBe(
      false,
    );
    engine.unregisterScope("modal");

    const traces = engine.getNavigationTrace();
    expect(
      traces.filter((record) => record.event === "CONTEXT_RESTORE_BEGIN"),
    ).toHaveLength(2);
    expect(
      traces.filter((record) => record.event === "CONTEXT_RESTORE_COMMIT"),
    ).toHaveLength(1);
    const begins = traces.filter(
      (record) => record.event === "CONTEXT_RESTORE_BEGIN",
    );
    expect(
      traces.find((record) => record.event === "CONTEXT_RESTORE_COMMIT")
        ?.transactionId,
    ).toBe(begins[1]?.transactionId);
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
    expect(engine.requestScopeRestore("modal", "root", "modal-to-root-2")).toBe(
      false,
    );
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
    expect(engine.dispatch("move-right")).toBe(true);

    registry.register({
      focusId: "details-play",
      scopeId: "details",
      element: addElement(new DOMRect()),
    });

    expect(engine.getActiveScopeId()).toBe("details");
    expect(engine.getActiveFocusId()).toBe("details-play");
    expect(engine.getScopeLifecycleState("details")).toBe("active");
  });

  it("requires an explicit fallback for a nonexistent restore target", () => {
    resetStore();
    const registry = new FocusRegistry();
    const engine = new NavigationEngine(registry, new FocusScrollManager());
    registry.register({
      focusId: "catalog-initial",
      scopeId: "catalog",
      element: addElement(new DOMRect()),
    });
    engine.registerScope({
      scopeId: "catalog",
      initialFocusId: "catalog-initial",
      activateOnMount: true,
    });

    expect(engine.activateScope("catalog", "missing-focus")).toBe(false);
    expect(engine.getScopeLifecycleState("catalog")).toBe(
      "waiting-for-focusable",
    );
    expect(engine.getActiveFocusId()).toBe("catalog-initial");
    expect(engine.completePendingRestore("catalog")).toBe(true);
    expect(engine.getActiveFocusId()).toBe("catalog-initial");

    const restoreCommits = engine
      .getNavigationTrace()
      .filter((record) => record.event === "CONTEXT_RESTORE_COMMIT");
    expect(restoreCommits).toHaveLength(1);
    expect(restoreCommits[0]).toMatchObject({
      selectedFocusId: "catalog-initial",
      focusReason: "region-fallback",
      restoreCommitCount: 1,
    });
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

  it("keeps the logical column after artwork results are replaced", () => {
    resetStore();
    const registry = new FocusRegistry();
    const engine = new NavigationEngine(registry, new FocusScrollManager());
    const registerCandidates = (prefix: string, count: number) => {
      const unregisters: Array<() => void> = [];
      for (let index = 0; index < count; index += 1) {
        unregisters.push(
          registry.register({
            focusId: `${prefix}-${index}`,
            scopeId: "artwork",
            element: addElement(
              new DOMRect(
                (index % 4) * 100,
                Math.floor(index / 4) * 60,
                80,
                40,
              ),
            ),
            gridNavigation: {
              groupId: "artwork-candidates",
              columns: 4,
              index,
              itemCount: count,
            },
          }),
        );
      }
      return () => unregisters.forEach((unregister) => unregister());
    };

    const unregisterInitial = registerCandidates("initial", 1);
    engine.registerScope({
      scopeId: "artwork",
      initialFocusId: "initial-0",
      activateOnMount: true,
    });
    unregisterInitial();

    for (const count of [7, 8, 9, 13, 17, 21, 25, 29, 33, 37, 41, 45, 49, 50]) {
      const unregister = registerCandidates(`results-${count}`, count);
      const focusIndex = 6;
      expect(engine.focus(`results-${count}-${focusIndex}`)).toBe(true);
      expect(engine.dispatch("move-up")).toBe(true);
      expect(engine.getActiveFocusId()).toBe(`results-${count}-2`);
      expect(engine.dispatch("move-down")).toBe(true);
      expect(engine.getActiveFocusId()).toBe(`results-${count}-${focusIndex}`);
      unregister();
    }

    engine.dispose();
  });

  it("keeps the details action row connected to the active tab", () => {
    resetStore();
    const registry = new FocusRegistry();
    const engine = new NavigationEngine(registry, new FocusScrollManager());
    const actionIds = [
      {
        id: "details-play",
        rect: new DOMRect(0, 0, 80, 40),
        navigation: { right: "details-favorite", down: "details-tab-activity" },
      },
      {
        id: "details-favorite",
        rect: new DOMRect(100, 0, 80, 40),
        navigation: {
          left: "details-play",
          right: "details-back",
          down: "details-tab-activity",
        },
      },
      {
        id: "details-back",
        rect: new DOMRect(200, 0, 80, 40),
        navigation: { left: "details-favorite", down: "details-tab-activity" },
      },
      {
        id: "details-tab-activity",
        rect: new DOMRect(100, 100, 120, 40),
        navigation: undefined,
      },
    ] as const;
    for (const entry of actionIds) {
      registry.register({
        focusId: entry.id,
        scopeId: "details",
        element: addElement(entry.rect),
        navigation: entry.navigation,
      });
    }
    engine.registerScope({
      scopeId: "details",
      initialFocusId: "details-play",
      activateOnMount: true,
    });

    expect(engine.dispatch("move-right")).toBe(true);
    expect(engine.getActiveFocusId()).toBe("details-favorite");
    expect(engine.dispatch("move-right")).toBe(true);
    expect(engine.getActiveFocusId()).toBe("details-back");
    expect(engine.dispatch("move-left")).toBe(true);
    expect(engine.getActiveFocusId()).toBe("details-favorite");
    expect(engine.dispatch("move-left")).toBe(true);
    expect(engine.getActiveFocusId()).toBe("details-play");
    expect(engine.dispatch("move-right")).toBe(true);
    expect(engine.dispatch("move-down")).toBe(true);
    expect(engine.getActiveFocusId()).toBe("details-tab-activity");
    engine.dispose();
  });

  it("updates gridIndex when a focusable is reused by a replacement result", () => {
    resetStore();
    const registry = new FocusRegistry();
    const engine = new NavigationEngine(registry, new FocusScrollManager());
    const register = (order: string[]) =>
      order.map((focusId, index) =>
        registry.register({
          focusId,
          scopeId: "artwork",
          element: addElement(
            new DOMRect((index % 4) * 100, Math.floor(index / 4) * 60, 80, 40),
          ),
          gridNavigation: {
            groupId: "artwork-candidates",
            columns: 4,
            index,
            itemCount: order.length,
          },
        }),
      );
    const firstOrder = [
      "candidate-a",
      "candidate-b",
      "candidate-c",
      "candidate-d",
      "candidate-e",
      "candidate-f",
      "candidate-reused",
      "candidate-h",
    ];
    const firstUnregisters = register(firstOrder);
    engine.registerScope({
      scopeId: "artwork",
      initialFocusId: "candidate-reused",
      activateOnMount: true,
    });
    expect(engine.getActiveFocusId()).toBe("candidate-reused");
    firstUnregisters.forEach((unregister) => unregister());

    const secondOrder = [
      "candidate-a",
      "candidate-b",
      "candidate-reused",
      "candidate-d",
      "candidate-e",
      "candidate-f",
      "candidate-h",
    ];
    const secondUnregisters = register(secondOrder);
    expect(engine.focus("candidate-reused")).toBe(true);
    expect(engine.dispatch("move-down")).toBe(true);
    expect(engine.getActiveFocusId()).toBe("candidate-h");
    expect(engine.dispatch("move-up")).toBe(true);
    expect(engine.getActiveFocusId()).toBe("candidate-reused");
    secondUnregisters.forEach((unregister) => unregister());
    engine.dispose();
  });

  it("honors an explicit grid border override before grid resolution", () => {
    resetStore();
    const registry = new FocusRegistry();
    const engine = new NavigationEngine(registry, new FocusScrollManager());
    registry.register({
      focusId: "grid-0",
      scopeId: "root",
      element: addElement(new DOMRect(0, 0, 80, 40)),
      gridNavigation: { groupId: "artwork-grid", columns: 4 },
      navigation: { down: "panel-action" },
    });
    registry.register({
      focusId: "grid-4",
      scopeId: "root",
      element: addElement(new DOMRect(0, 60, 80, 40)),
      gridNavigation: { groupId: "artwork-grid", columns: 4 },
    });
    registry.register({
      focusId: "panel-action",
      scopeId: "root",
      element: addElement(new DOMRect(500, 60, 80, 40)),
    });
    engine.registerScope({
      scopeId: "root",
      initialFocusId: "grid-0",
      activateOnMount: true,
    });

    expect(engine.dispatch("move-down")).toBe(true);
    expect(engine.getActiveFocusId()).toBe("panel-action");
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
      expect(
        engine.requestScopeRestore(
          "details",
          "home",
          `details-to-home-${rowIndex}`,
        ),
      ).toBe(false);
      engine.unregisterScope("details");
      expect(engine.getActiveFocusId()).toBe(opener);
      if (rowIndex < 2) {
        expect(engine.dispatch("move-down")).toBe(true);
        expect(engine.getActiveFocusId()).toBe(`home-${rowIndex + 1}-2`);
      }
    }
  });

  it("preserves row memory across an equivalent route restore", () => {
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
      initialFocusId: "home-1-0",
      activateOnMount: true,
    });

    const openAndClose = (opener: string) => {
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
      expect(
        engine.requestScopeRestore(
          "details",
          "home",
          `details-to-home-${opener}`,
        ),
      ).toBe(false);
      engine.unregisterScope("details");
      expect(engine.getActiveFocusId()).toBe(opener);
    };

    openAndClose("home-1-1");
    expect(engine.dispatch("move-up")).toBe(true);
    expect(engine.getActiveFocusId()).toBe("home-0-1");

    openAndClose("home-1-2");
    expect(engine.dispatch("move-up")).toBe(true);
    expect(engine.getActiveFocusId()).toBe("home-0-2");

    const verticalResolution = engine
      .getNavigationTrace()
      .find(
        (record) =>
          record.event === "NAV_RESOLVE" &&
          record.direction === "up" &&
          record.fromFocusId === "home-1-2",
      );
    expect(verticalResolution).toMatchObject({
      fromItemIndex: 2,
      preferredItemIndexBefore: 2,
      selectedFocusId: "home-0-2",
      selectedItemIndex: 2,
      resolutionStrategy: "row-memory-rejected",
      memoryDecision: "rejected",
      memoryRejectionReason: "generation-mismatch",
    });
  });

  it.each(["gamepad", "mouse"] as const)(
    "preserves the logical vertical result for %s selection across restore",
    (inputSource) => {
      const run = (
        restore: boolean,
        itemIndex: number,
        direction: "up" | "down",
      ) => {
        resetStore();
        const registry = new FocusRegistry();
        const engine = new NavigationEngine(registry, new FocusScrollManager());
        const prefix = `${inputSource}-${restore ? "restore" : "direct"}-${itemIndex}-${direction}`;
        for (let rowIndex = 0; rowIndex < 3; rowIndex += 1) {
          for (
            let currentItemIndex = 0;
            currentItemIndex < 5;
            currentItemIndex += 1
          ) {
            registry.register({
              focusId: `${prefix}-row-${rowIndex}-${currentItemIndex}`,
              scopeId: `${prefix}-catalog`,
              element: addElement(
                new DOMRect(currentItemIndex * 100, rowIndex * 100, 80, 40),
              ),
              rowNavigation: {
                groupId: `${prefix}-rows`,
                rowId: `${prefix}-row-${rowIndex}`,
                rowIndex,
                itemIndex: currentItemIndex,
                preserveHorizontalIntent: true,
              },
            });
          }
        }
        registry.register({
          focusId: `${prefix}-details-action`,
          scopeId: `${prefix}-details`,
          element: addElement(new DOMRect()),
        });
        const catalogScopeId = `${prefix}-catalog`;
        const openerFocusId = `${prefix}-row-1-${itemIndex}`;
        engine.registerScope({
          scopeId: catalogScopeId,
          initialFocusId: `${prefix}-row-1-0`,
          activateOnMount: true,
        });
        if (inputSource === "gamepad") {
          for (
            let currentItemIndex = 0;
            currentItemIndex < itemIndex;
            currentItemIndex += 1
          ) {
            expect(engine.dispatch("move-right", "gamepad")).toBe(true);
          }
        } else {
          expect(engine.focusFromPointer(openerFocusId)).toBe(true);
        }
        if (restore) {
          engine.prepareScopeOpen(`${prefix}-details`, openerFocusId);
          engine.registerScope({
            scopeId: `${prefix}-details`,
            parentScopeId: catalogScopeId,
            initialFocusId: `${prefix}-details-action`,
            modal: true,
            activateOnMount: true,
            restoreFocus: true,
          });
          expect(
            engine.requestScopeRestore(
              `${prefix}-details`,
              catalogScopeId,
              `${prefix}-details-to-catalog`,
            ),
          ).toBe(false);
          engine.unregisterScope(`${prefix}-details`);
        }
        expect(engine.dispatch(`move-${direction}`, inputSource)).toBe(true);
        const targetRowIndex = direction === "up" ? 0 : 2;
        expect(engine.getActiveFocusId()).toBe(
          `${prefix}-row-${targetRowIndex}-${itemIndex}`,
        );
        engine.dispose();
      };

      for (const itemIndex of [1, 2, 3, 4]) {
        for (const direction of ["up", "down"] as const) {
          run(false, itemIndex, direction);
          run(true, itemIndex, direction);
        }
      }
    },
  );

  it("reproduces the missing Home tab bridge from every first-row card", () => {
    resetStore();
    const registry = new FocusRegistry();
    const engine = new NavigationEngine(registry, new FocusScrollManager());
    registry.register({
      focusId: "main-nav-home",
      scopeId: "product-shell",
      element: addElement(new DOMRect(0, 0, 80, 40)),
      linearNavigation: {
        groupId: "main-navigation",
        axis: "horizontal",
      },
      navigationRegion: {
        regionId: "main-navigation",
        childRegionId: "home-content",
        entryFocusId: "home-continue-0",
      },
    });
    registry.register({
      focusId: "main-nav-library",
      scopeId: "product-shell",
      element: addElement(new DOMRect(100, 0, 80, 40)),
      linearNavigation: {
        groupId: "main-navigation",
        axis: "horizontal",
      },
      navigationRegion: {
        regionId: "main-navigation",
        childRegionId: "library-content",
        entryFocusId: "library-0",
      },
    });
    for (let itemIndex = 0; itemIndex < 5; itemIndex += 1) {
      registry.register({
        focusId: `home-continue-${itemIndex}`,
        scopeId: "product-shell",
        element: addElement(new DOMRect(itemIndex * 100, 100, 80, 40)),
        rowNavigation: {
          groupId: "home-rows",
          rowId: "home-continue",
          rowIndex: 0,
          itemIndex,
          preserveHorizontalIntent: true,
        },
        navigationRegion: {
          regionId: "home-content",
          parentRegionId: "main-navigation",
          entryFocusId: "main-nav-home",
          exitFocusId: "main-nav-home",
        },
      });
    }
    engine.registerScope({
      scopeId: "product-shell",
      initialFocusId: "main-nav-home",
      activateOnMount: true,
    });

    for (let itemIndex = 0; itemIndex < 5; itemIndex += 1) {
      expect(engine.focus(`home-continue-${itemIndex}`)).toBe(true);
      expect(engine.dispatch("move-up")).toBe(true);
      expect(engine.getActiveFocusId()).toBe("main-nav-home");
      expect(engine.dispatch("move-down")).toBe(true);
      expect(engine.getActiveFocusId()).toBe(`home-continue-${itemIndex}`);
    }
  });
});
