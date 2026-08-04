import { StrictMode, act, useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { describe, expect, it } from "vitest";
import App from "../App";
import { useLibraryStore } from "../stores/library-store";
import { useNavigationStore } from "../stores/navigation-store";
import { useProductStore } from "../stores/product-store";
import { useNavigation } from "../ui/navigation/navigation-context";
import { FocusScope } from "../ui/navigation/focus/FocusScope";
import { Focusable } from "../ui/navigation/focus/Focusable";
import { NavigationProvider } from "../ui/navigation/NavigationProvider";
import { NavigationGrid } from "../ui/navigation/layouts/NavigationGrid";
import { NavigationEngine } from "../ui/navigation/core/navigation-engine";
import { navigationRuntimeTrace } from "../ui/navigation/debug/navigation-runtime-trace";

async function waitForCatalog(): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

interface MutableGamepad extends Omit<Gamepad, "axes" | "buttons"> {
  axes: number[];
  buttons: Array<{
    pressed: boolean;
    touched: boolean;
    value: number;
  }>;
}

function makeTriggerGamepad(): MutableGamepad {
  const buttons = Array.from({ length: 8 }, () => ({
    pressed: false,
    touched: false,
    value: 0,
  }));
  return {
    axes: [0, 0, 0, 0, 0, 0],
    buttons,
    connected: true,
    id: "integration-trigger-pad",
    index: 0,
    mapping: "standard",
    timestamp: 0,
    vibrationActuator: {
      playEffect: async () => "complete",
      reset: async () => "complete",
    },
  };
}

function setTrigger(
  gamepad: MutableGamepad,
  buttonIndex: number,
  value: number,
): void {
  const button = gamepad.buttons[buttonIndex];
  if (!button) return;
  button.value = value;
  button.pressed = value >= 0.75;
}

async function waitForGamepadPoll(): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 30));
}

async function waitForSelector(
  host: HTMLElement,
  selector: string,
): Promise<void> {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (host.querySelector(selector)) return;
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
  throw new Error(`Selector did not appear: ${selector}`);
}

async function dispatchKey(key: string): Promise<void> {
  window.dispatchEvent(new KeyboardEvent("keydown", { key }));
  window.dispatchEvent(new KeyboardEvent("keyup", { key }));
  await new Promise((resolve) => setTimeout(resolve, 0));
}

async function renderProductApp() {
  useLibraryStore.getState().reset();
  useProductStore.setState({
    activeView: "home",
    selectedGameId: null,
    returnView: "home",
    returnFocusId: null,
  });
  useNavigationStore.setState({
    activeScopeId: null,
    activeFocusId: null,
    previousFocusId: null,
    lastNavigationAction: null,
  });
  const host = document.createElement("div");
  document.body.appendChild(host);
  const root = createRoot(host);
  await act(async () => {
    root.render(
      <StrictMode>
        <App />
      </StrictMode>,
    );
  });
  await act(waitForCatalog);
  return { host, root };
}

function PendingDetailsHarness({
  delayFrames,
  onEngine,
  releaseDetails,
}: {
  delayFrames: number;
  onEngine: (engine: NavigationEngine) => void;
  releaseDetails: { current: (() => void) | null };
}) {
  const { engine } = useNavigation();
  const [view, setView] = useState<"library" | "details">("library");
  const [detailsReady, setDetailsReady] = useState(delayFrames === 0);
  const transitionIdRef = useRef(0);

  useEffect(() => onEngine(engine), [engine, onEngine]);
  useEffect(() => {
    releaseDetails.current =
      view === "details" ? () => setDetailsReady(true) : null;
    if (view !== "details") setDetailsReady(false);
    return () => {
      releaseDetails.current = null;
    };
  }, [releaseDetails, view]);

  const openDetails = () => {
    engine.prepareScopeOpen("details", "library-game-0");
    setView("details");
  };

  const closeDetails = () => {
    transitionIdRef.current += 1;
    engine.requestScopeRestore(
      "details",
      "product-shell",
      `details-to-library-${transitionIdRef.current}`,
    );
    setView("library");
  };

  return (
    <FocusScope
      scopeId="product-shell"
      initialFocusId="library-game-0"
      activateOnMount
    >
      <Focusable focusId="shell-anchor" scopeId="product-shell">
        Shell
      </Focusable>
      {view === "library" ? (
        <NavigationGrid
          groupId="library-grid"
          columns={5}
          itemCount={10}
          resolveFocusId={(index) => `library-game-${index}`}
          onRequestIndex={() => undefined}
        >
          <Focusable
            focusId="library-game-0"
            scopeId="product-shell"
            gridIndex={0}
            onConfirm={openDetails}
          >
            Open Details
          </Focusable>
        </NavigationGrid>
      ) : (
        <FocusScope
          scopeId="details"
          parentScopeId="product-shell"
          initialFocusId="details-play"
          activateOnMount
          modal
          trapFocus
        >
          {detailsReady && (
            <>
              <Focusable focusId="details-play" scopeId="details">
                Play
              </Focusable>
              <Focusable
                focusId="details-back"
                scopeId="details"
                onConfirm={closeDetails}
              >
                Back
              </Focusable>
            </>
          )}
        </FocusScope>
      )}
    </FocusScope>
  );
}

describe("LumaDeck product slice integration", () => {
  it("mounts a persistent shell with a stable home focus", async () => {
    const { host, root } = await renderProductApp();

    expect(host.querySelector(".app-shell")).not.toBeNull();
    expect(
      host.querySelector('[data-focus-id="main-nav-home"]'),
    ).not.toBeNull();
    expect(
      host.querySelectorAll('[data-focusable="true"]').length,
    ).toBeGreaterThan(10);
    expect(host.querySelector('[data-active="true"]')).not.toBeNull();

    await act(async () => root.unmount());
  });

  it("bridges main tabs and remembers the last content focus", async () => {
    const { host, root } = await renderProductApp();
    const homeCard = host.querySelector<HTMLElement>(
      '[data-focus-id^="home-continue-"]',
    );
    expect(homeCard).not.toBeNull();

    await act(async () => dispatchKey("ArrowDown"));
    expect(homeCard?.getAttribute("data-active")).toBe("true");

    const recentCard = host.querySelector<HTMLElement>(
      '[data-focus-id^="home-recent-"]',
    );
    expect(recentCard).not.toBeNull();
    await act(async () => dispatchKey("ArrowDown"));
    expect(recentCard?.getAttribute("data-active")).toBe("true");
    await act(async () => dispatchKey("ArrowUp"));
    expect(homeCard?.getAttribute("data-active")).toBe("true");
    await act(async () => dispatchKey("ArrowUp"));
    expect(
      host
        .querySelector('[data-focus-id="main-nav-home"]')
        ?.getAttribute("data-active"),
    ).toBe("true");

    await act(async () => dispatchKey("ArrowDown"));
    expect(homeCard?.getAttribute("data-active")).toBe("true");

    await act(async () => dispatchKey("ArrowUp"));
    await act(async () => dispatchKey("ArrowRight"));
    expect(
      host
        .querySelector('[data-focus-id="main-nav-library"]')
        ?.getAttribute("data-active"),
    ).toBe("true");
    await act(async () => dispatchKey("Enter"));
    await act(async () => waitForSelector(host, "#library-heading"));
    await act(async () => dispatchKey("ArrowDown"));
    const libraryCard = host.querySelector<HTMLElement>(
      '[data-focus-id="library-game-001"]',
    );
    expect(libraryCard).not.toBeNull();
    expect(libraryCard?.getAttribute("data-active")).toBe("true");
    await act(async () => dispatchKey("ArrowUp"));
    expect(
      host
        .querySelector('[data-focus-id="main-nav-library"]')
        ?.getAttribute("data-active"),
    ).toBe("true");
    await act(async () => dispatchKey("ArrowDown"));
    expect(libraryCard?.getAttribute("data-active")).toBe("true");

    await act(async () => root.unmount());
  });

  it("restores the first Recently Played card before navigating Up", async () => {
    const { host, root } = await renderProductApp();

    await act(async () => dispatchKey("ArrowDown"));
    await act(async () => dispatchKey("ArrowDown"));
    const recentFirst = host.querySelector<HTMLElement>(
      '[data-focus-id="home-recent-game-002"]',
    );
    expect(recentFirst?.getAttribute("data-active")).toBe("true");

    await act(async () => recentFirst?.click());
    await act(async () => waitForSelector(host, "#details-heading"));
    const detailsBack = host.querySelector<HTMLElement>(
      '[data-focus-id="details-back"]',
    );
    expect(detailsBack).not.toBeNull();
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 100));
    });
    await act(async () => detailsBack?.click());

    const viewAfterBack = host
      .querySelector("[data-view]")
      ?.getAttribute("data-view");
    const restoredFocusId = host
      .querySelector('[data-active="true"]')
      ?.getAttribute("data-focus-id");
    await act(async () => dispatchKey("ArrowUp"));
    const focusAfterUp = host
      .querySelector('[data-active="true"]')
      ?.getAttribute("data-focus-id");

    expect({ viewAfterBack, restoredFocusId, focusAfterUp }).toEqual({
      viewAfterBack: "home",
      restoredFocusId: "home-recent-game-002",
      focusAfterUp: "home-continue-game-002",
    });

    await act(async () => root.unmount());
  });

  it.each([
    {
      column: 0,
      up: "home-continue-game-002",
      down: "home-favorite-game-001",
    },
    {
      column: 1,
      up: "home-continue-game-005",
      down: "home-favorite-game-008",
    },
    {
      column: 2,
      up: "home-continue-game-008",
      down: "home-favorite-game-015",
    },
    {
      column: 3,
      up: "home-continue-game-011",
      down: "home-favorite-game-022",
    },
  ])(
    "restores Recently Played column $column before navigating vertically",
    async ({ column, up, down }) => {
      for (const [direction, expectedFocusId] of [
        ["ArrowUp", up],
        ["ArrowDown", down],
      ] as const) {
        const { host, root } = await renderProductApp();

        await act(async () => dispatchKey("ArrowDown"));
        await act(async () => dispatchKey("ArrowDown"));
        for (let step = 0; step < column; step += 1) {
          await act(async () => dispatchKey("ArrowRight"));
        }
        const recentCard = host.querySelector<HTMLElement>(
          '[data-focus-id^="home-recent-"][data-active="true"]',
        );
        expect(recentCard).not.toBeNull();
        await act(async () => recentCard?.click());
        await act(async () => waitForSelector(host, "#details-heading"));
        await act(async () => {
          await new Promise((resolve) => setTimeout(resolve, 100));
        });
        await act(async () =>
          host
            .querySelector<HTMLElement>('[data-focus-id="details-back"]')
            ?.click(),
        );
        await act(async () => waitForSelector(host, "#home-heading"));
        await act(async () => dispatchKey(direction));

        expect(
          host
            .querySelector('[data-active="true"]')
            ?.getAttribute("data-focus-id"),
        ).toBe(expectedFocusId);

        await act(async () => root.unmount());
      }
    },
  );

  it("switches to Library without unmounting the shell and opens Details", async () => {
    const { host, root } = await renderProductApp();
    const shell = host.querySelector(".app-shell");
    const libraryButton = host.querySelector<HTMLElement>(
      '[data-focus-id="main-nav-library"]',
    );
    expect(libraryButton).not.toBeNull();

    await act(async () => useProductStore.getState().setView("library"));
    await act(async () => waitForSelector(host, "#library-heading"));
    expect(host.querySelector(".app-shell")).toBe(shell);
    await act(async () => waitForSelector(host, "#library-heading"));
    expect(host.querySelector("#library-heading")).not.toBeNull();
    expect(
      host
        .querySelector('[data-focus-id="main-nav-library"]')
        ?.getAttribute("data-active"),
    ).toBe("true");
    expect(
      host.querySelectorAll('[data-focus-id^="library-game-"]').length,
    ).toBeLessThanOrEqual(60);

    const firstGame = host.querySelector<HTMLElement>(
      '[data-focus-id="library-game-001"]',
    );
    expect(firstGame).not.toBeNull();
    await act(async () => firstGame?.click());
    expect(host.querySelector("#details-heading")).not.toBeNull();
    expect(
      host
        .querySelector('[data-focus-id="details-play"]')
        ?.getAttribute("data-active"),
    ).toBe("true");

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 100));
    });
    await act(async () =>
      host
        .querySelector<HTMLElement>('[data-focus-id="details-back"]')
        ?.click(),
    );
    await act(async () => waitForSelector(host, "#library-heading"));
    expect(host.querySelector("#library-heading")).not.toBeNull();
    expect(
      host
        .querySelector('[data-focus-id="library-game-001"]')
        ?.getAttribute("data-active"),
    ).toBe("true");

    await act(async () => root.unmount());
  });

  it("navigates Home and Library through the real input manager trigger path", async () => {
    const gamepad = makeTriggerGamepad();
    const originalGetGamepads = Object.getOwnPropertyDescriptor(
      Navigator.prototype,
      "getGamepads",
    );
    Object.defineProperty(navigator, "getGamepads", {
      configurable: true,
      value: () => [gamepad],
    });
    const { host, root } = await renderProductApp();

    try {
      await act(async () => dispatchKey("ArrowDown"));
      expect(useNavigationStore.getState().activeFocusId).toMatch(/^home-/);
      setTrigger(gamepad, 7, 0.8);
      await act(async () => waitForSelector(host, "#library-heading"));
      expect(useNavigationStore.getState().activeFocusId).not.toBeNull();

      setTrigger(gamepad, 7, 0.4);
      await act(async () => waitForGamepadPoll());
      await act(async () => dispatchKey("ArrowDown"));
      expect(useNavigationStore.getState().activeFocusId).toMatch(/^library-/);

      setTrigger(gamepad, 6, 0.8);
      await act(async () => waitForGamepadPoll());
      await act(async () => waitForSelector(host, "#home-heading"));
      expect(useNavigationStore.getState().activeFocusId).toMatch(/^home-/);
    } finally {
      await act(async () => root.unmount());
      if (originalGetGamepads) {
        Object.defineProperty(
          Navigator.prototype,
          "getGamepads",
          originalGetGamepads,
        );
      } else {
        Object.defineProperty(navigator, "getGamepads", {
          configurable: true,
          value: undefined,
        });
      }
    }
  });

  it("blocks triggers in Details and enables them again after restore", async () => {
    const gamepad = makeTriggerGamepad();
    const originalGetGamepads = Object.getOwnPropertyDescriptor(
      Navigator.prototype,
      "getGamepads",
    );
    Object.defineProperty(navigator, "getGamepads", {
      configurable: true,
      value: () => [gamepad],
    });
    const { host, root } = await renderProductApp();

    try {
      await act(async () =>
        host
          .querySelector<HTMLElement>('[data-focus-id^="home-continue-"]')
          ?.click(),
      );
      await act(async () => waitForSelector(host, "#details-heading"));

      setTrigger(gamepad, 7, 0.8);
      await act(async () => waitForGamepadPoll());
      expect(host.querySelector("#details-heading")).not.toBeNull();
      expect(useProductStore.getState().activeView).toBe("details");

      setTrigger(gamepad, 7, 0.4);
      await act(async () => waitForGamepadPoll());
      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 100));
        host
          .querySelector<HTMLElement>('[data-focus-id="details-back"]')
          ?.click();
      });
      await act(async () => waitForSelector(host, "#home-heading"));

      setTrigger(gamepad, 7, 0.8);
      await act(async () => waitForSelector(host, "#library-heading"));
      expect(useProductStore.getState().activeView).toBe("library");
    } finally {
      await act(async () => root.unmount());
      if (originalGetGamepads) {
        Object.defineProperty(
          Navigator.prototype,
          "getGamepads",
          originalGetGamepads,
        );
      } else {
        Object.defineProperty(navigator, "getGamepads", {
          configurable: true,
          value: undefined,
        });
      }
    }
  });

  it.each(["home", "library"] as const)(
    "runs the minimal gamepad Details back contract for %s",
    async (screen) => {
      const gamepad = makeTriggerGamepad();
      const originalGetGamepads = Object.getOwnPropertyDescriptor(
        Navigator.prototype,
        "getGamepads",
      );
      Object.defineProperty(navigator, "getGamepads", {
        configurable: true,
        value: () => [gamepad],
      });
      let engine: NavigationEngine | undefined;
      const captureEngine = (value: NavigationEngine): void => {
        engine = value;
      };
      const originalFocus = NavigationEngine.prototype.focus;
      const originalDispatch = NavigationEngine.prototype.dispatch;
      NavigationEngine.prototype.focus = function (
        focusId: Parameters<NavigationEngine["focus"]>[0],
        activateScope: Parameters<NavigationEngine["focus"]>[1] = true,
        options: Parameters<NavigationEngine["focus"]>[2],
      ): ReturnType<NavigationEngine["focus"]> {
        captureEngine(this);
        return originalFocus.call(this, focusId, activateScope, options);
      };
      NavigationEngine.prototype.dispatch = function (
        action: Parameters<NavigationEngine["dispatch"]>[0],
        inputSource: Parameters<
          NavigationEngine["dispatch"]
        >[1] = "programmatic",
      ): ReturnType<NavigationEngine["dispatch"]> {
        captureEngine(this);
        return originalDispatch.call(this, action, inputSource);
      };
      const { host, root } = await renderProductApp();
      const pressButton = async (buttonIndex: number): Promise<void> => {
        setTrigger(gamepad, buttonIndex, 1);
        await act(async () => waitForGamepadPoll());
        setTrigger(gamepad, buttonIndex, 0);
        await act(async () => waitForGamepadPoll());
      };
      const pressDown = async (): Promise<void> => {
        gamepad.axes[1] = 1;
        await act(async () => waitForGamepadPoll());
        gamepad.axes[1] = 0;
        await act(async () => waitForGamepadPoll());
      };

      try {
        if (screen === "library") {
          await act(async () => useProductStore.getState().setView("library"));
          await act(async () => waitForSelector(host, "#library-heading"));
        } else {
          await act(async () => waitForSelector(host, "#home-heading"));
        }

        for (let attempt = 0; attempt < 4; attempt += 1) {
          const activeFocusId = useNavigationStore.getState().activeFocusId;
          const onCard =
            screen === "home"
              ? activeFocusId?.startsWith("home-")
              : activeFocusId?.startsWith("library-game-");
          if (onCard) break;
          await pressDown();
        }
        const openerFocusId = useNavigationStore.getState().activeFocusId;
        expect(openerFocusId).toMatch(
          screen === "home" ? /^home-/ : /^library-game-/,
        );

        await pressButton(0);
        await act(async () => waitForSelector(host, "#details-heading"));
        await pressButton(1);
        await act(async () => waitForSelector(host, `#${screen}-heading`));

        const stateAfterBack = useNavigationStore.getState();
        if (!engine) throw new Error("Navigation engine was not captured");
        const activeEntry = stateAfterBack.activeFocusId
          ? engine.registry.get(stateAfterBack.activeFocusId)
          : undefined;
        expect(useProductStore.getState().activeView).toBe(screen);
        expect(stateAfterBack.activeScopeId).toBe("product-shell");
        expect(stateAfterBack.activeFocusId).not.toBeNull();
        expect(activeEntry).toBeDefined();
        expect(activeEntry?.disabled).not.toBe(true);
        expect(activeEntry?.hidden).not.toBe(true);
        expect(activeEntry?.scopeId).toBe(stateAfterBack.activeScopeId);
        expect(activeEntry?.navigationRegion?.regionId).toBe(
          screen === "home" ? "home-content" : "library-content",
        );
        expect(document.activeElement).toBe(activeEntry?.element);

        const traceStart = engine.getNavigationTrace().length;
        await pressDown();
        const firstDirectionalInput = engine
          .getNavigationTrace()
          .slice(traceStart)
          .find(
            (record) =>
              record.event === "NAV_INPUT" && record.direction === "down",
          );
        const firstResolve = engine
          .getNavigationTrace()
          .slice(traceStart)
          .find((record) => record.event === "NAV_RESOLVE");
        expect(firstDirectionalInput).toBeDefined();
        expect(firstResolve).toBeDefined();
        expect(firstResolve?.selectedFocusId).not.toBeNull();
        expect(
          engine.registry.get(firstResolve?.selectedFocusId ?? ""),
        ).toBeDefined();
      } finally {
        await act(async () => root.unmount());
        NavigationEngine.prototype.focus = originalFocus;
        NavigationEngine.prototype.dispatch = originalDispatch;
        if (originalGetGamepads) {
          Object.defineProperty(
            Navigator.prototype,
            "getGamepads",
            originalGetGamepads,
          );
        } else {
          Object.defineProperty(navigator, "getGamepads", {
            configurable: true,
            value: undefined,
          });
        }
      }
    },
  );

  it("restores gamepad focus after a fast back from a filtered Details view", async () => {
    const gamepad = makeTriggerGamepad();
    const originalGetGamepads = Object.getOwnPropertyDescriptor(
      Navigator.prototype,
      "getGamepads",
    );
    Object.defineProperty(navigator, "getGamepads", {
      configurable: true,
      value: () => [gamepad],
    });
    const { host, root } = await renderProductApp();

    try {
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
        setter?.call(input, "Beacon");
        await act(async () => {
          input.dispatchEvent(new Event("input", { bubbles: true }));
          await waitForSelector(host, '[data-focus-id^="library-game-"]');
        });
      }

      await act(async () => {
        host
          .querySelector<HTMLElement>('[data-focus-id^="library-game-"]')
          ?.click();
      });
      await act(async () => waitForSelector(host, "#details-heading"));

      const backButton = gamepad.buttons[1];
      if (!backButton) throw new Error("Gamepad back button is unavailable");
      backButton.pressed = true;
      backButton.value = 1;
      gamepad.axes[0] = 1;
      await act(async () => waitForGamepadPoll());
      backButton.pressed = false;
      backButton.value = 0;
      gamepad.axes[0] = 0;
      await act(async () => waitForGamepadPoll());

      await act(async () => waitForSelector(host, "#library-heading"));
      expect(useNavigationStore.getState().activeScopeId).toBe("product-shell");
      expect(useNavigationStore.getState().activeFocusId).not.toBeNull();
      expect(document.activeElement?.getAttribute("data-focus-id")).not.toBe(
        null,
      );
    } finally {
      await act(async () => root.unmount());
      if (originalGetGamepads) {
        Object.defineProperty(
          Navigator.prototype,
          "getGamepads",
          originalGetGamepads,
        );
      } else {
        Object.defineProperty(navigator, "getGamepads", {
          configurable: true,
          value: undefined,
        });
      }
    }
  });

  it("captures the complete virtual-keyboard search to fast Details back flow", async () => {
    const gamepad = makeTriggerGamepad();
    const originalGetGamepads = Object.getOwnPropertyDescriptor(
      Navigator.prototype,
      "getGamepads",
    );
    Object.defineProperty(navigator, "getGamepads", {
      configurable: true,
      value: () => [gamepad],
    });
    let engine: NavigationEngine | undefined;
    const captureEngine = (value: NavigationEngine): void => {
      engine = value;
    };
    const originalFocus = NavigationEngine.prototype.focus;
    const originalDispatch = NavigationEngine.prototype.dispatch;
    NavigationEngine.prototype.focus = function (
      focusId: Parameters<NavigationEngine["focus"]>[0],
      activateScope: Parameters<NavigationEngine["focus"]>[1] = true,
      options: Parameters<NavigationEngine["focus"]>[2],
    ): ReturnType<NavigationEngine["focus"]> {
      captureEngine(this);
      return originalFocus.call(this, focusId, activateScope, options);
    };
    NavigationEngine.prototype.dispatch = function (
      action: Parameters<NavigationEngine["dispatch"]>[0],
      inputSource: Parameters<NavigationEngine["dispatch"]>[1] = "programmatic",
    ): ReturnType<NavigationEngine["dispatch"]> {
      captureEngine(this);
      return originalDispatch.call(this, action, inputSource);
    };
    useProductStore.setState({
      activeView: "home",
      selectedGameId: null,
      returnView: "home",
      returnFocusId: null,
    });
    useLibraryStore.getState().reset();
    useNavigationStore.setState({
      activeScopeId: null,
      activeFocusId: null,
      previousFocusId: null,
      lastNavigationAction: null,
    });
    const host = document.createElement("div");
    document.body.appendChild(host);
    const root = createRoot(host);

    const pressButton = async (buttonIndex: number): Promise<void> => {
      setTrigger(gamepad, buttonIndex, 1);
      await act(async () => waitForGamepadPoll());
      setTrigger(gamepad, buttonIndex, 0);
      await act(async () => waitForGamepadPoll());
    };
    const pressDown = async (): Promise<void> => {
      gamepad.axes[1] = 1;
      await act(async () => waitForGamepadPoll());
      gamepad.axes[1] = 0;
      await act(async () => waitForGamepadPoll());
    };
    const pressLeft = async (): Promise<void> => {
      gamepad.axes[0] = -1;
      await act(async () => waitForGamepadPoll());
      gamepad.axes[0] = 0;
      await act(async () => waitForGamepadPoll());
    };
    const pressRight = async (): Promise<void> => {
      gamepad.axes[0] = 1;
      await act(async () => waitForGamepadPoll());
      gamepad.axes[0] = 0;
      await act(async () => waitForGamepadPoll());
    };
    const pressConfirmAndImmediateDown = async (): Promise<void> => {
      setTrigger(gamepad, 0, 1);
      await act(async () => waitForGamepadPoll());
      setTrigger(gamepad, 0, 0);
      gamepad.axes[1] = 1;
      await act(async () => waitForGamepadPoll());
      gamepad.axes[1] = 0;
      await act(async () => waitForGamepadPoll());
    };

    try {
      await act(async () => {
        root.render(
          <StrictMode>
            <App />
          </StrictMode>,
        );
      });
      await act(async () => useProductStore.getState().setView("library"));
      await act(async () => waitForSelector(host, "#library-heading"));
      await act(async () =>
        waitForSelector(host, '[data-focus-id="library-game-001"]'),
      );
      expect(engine?.focus("library-filter-all")).toBe(true);

      const searchInput = host.querySelector<HTMLInputElement>(
        '[data-focus-id="library-search"]',
      );
      expect(searchInput).not.toBeNull();
      for (let attempt = 0; attempt < 3; attempt += 1) {
        if (useNavigationStore.getState().activeFocusId === "library-search") {
          break;
        }
        await pressLeft();
      }
      expect(useNavigationStore.getState().activeFocusId).toBe(
        "library-search",
      );

      await pressButton(0);
      await act(async () =>
        waitForSelector(host, '[data-keyboard-modal="true"]'),
      );
      expect(useNavigationStore.getState().activeScopeId).toBe(
        "virtual-keyboard",
      );

      for (const keyId of ["b", "e", "a", "c", "o", "n"]) {
        const key = host.querySelector<HTMLElement>(
          `[data-focus-id="virtual-key-${keyId}"]`,
        );
        expect(key).not.toBeNull();
        expect(engine?.focus(`virtual-key-${keyId}`)).toBe(true);
        await pressButton(0);
      }
      expect(host.querySelector<HTMLOutputElement>("output")?.textContent).toBe(
        "beacon",
      );

      await pressButton(7);
      await act(async () => waitForSelector(host, "#library-heading"));
      expect(host.querySelector('[data-keyboard-modal="true"]')).toBeNull();
      expect(useNavigationStore.getState().activeFocusId).toBe(
        "library-search",
      );
      expect(document.activeElement).toBe(searchInput);
      const filteredCardCount = host.querySelectorAll(
        '[data-focus-id^="library-game-"]',
      ).length;
      expect(
        filteredCardCount,
        `draft=${host.querySelector("output")?.textContent ?? ""}; input=${host.querySelector<HTMLInputElement>("[data-gamepad-text-input]")?.value ?? ""}; hint=${host.querySelector(".page-hint")?.textContent ?? ""}`,
      ).toBeGreaterThan(0);
      expect(
        host.querySelectorAll('[data-focus-id^="library-game-"]').length,
      ).toBeLessThan(200);

      for (let attempt = 0; attempt < 6; attempt += 1) {
        if (
          useNavigationStore
            .getState()
            .activeFocusId?.startsWith("library-game-")
        ) {
          break;
        }
        await pressDown();
      }
      expect(
        useNavigationStore
          .getState()
          .activeFocusId?.startsWith("library-game-"),
      ).toBe(true);
      await pressRight();
      await pressRight();
      expect(
        useNavigationStore
          .getState()
          .activeFocusId?.startsWith("library-game-"),
      ).toBe(true);

      await pressButton(0);
      await act(async () => waitForSelector(host, "#details-heading"));
      await pressDown();
      expect(useNavigationStore.getState().activeFocusId).toBe("details-back");
      const traceLengthBeforeBack = engine?.getNavigationTrace().length ?? 0;
      await pressConfirmAndImmediateDown();
      await act(async () => waitForSelector(host, "#library-heading"));

      const viewAfterBack = useProductStore.getState().activeView;
      const stateAfterBack = useNavigationStore.getState();
      const activeEntry = stateAfterBack.activeFocusId
        ? engine?.registry.get(stateAfterBack.activeFocusId)
        : undefined;
      expect(viewAfterBack).toBe("library");
      expect(stateAfterBack.activeFocusId).not.toBeNull();
      expect(activeEntry).toBeDefined();
      expect(activeEntry?.element.isConnected).toBe(true);
      expect(activeEntry?.disabled).not.toBe(true);
      expect(activeEntry?.hidden).not.toBe(true);
      expect(activeEntry?.scopeId).toBe(stateAfterBack.activeScopeId);
      expect(document.activeElement).toBe(activeEntry?.element);

      const traceLengthBeforeMove = traceLengthBeforeBack;
      const firstResolve = engine
        ?.getNavigationTrace()
        .slice(traceLengthBeforeMove)
        .find((record) => record.event === "NAV_RESOLVE");
      expect(firstResolve).toBeDefined();
    } finally {
      await act(async () => root.unmount());
      NavigationEngine.prototype.focus = originalFocus;
      NavigationEngine.prototype.dispatch = originalDispatch;
      if (originalGetGamepads) {
        Object.defineProperty(
          Navigator.prototype,
          "getGamepads",
          originalGetGamepads,
        );
      } else {
        Object.defineProperty(navigator, "getGamepads", {
          configurable: true,
          value: undefined,
        });
      }
    }
  });

  it("preserves filtered Library content across a fast Details back", async () => {
    const gamepad = makeTriggerGamepad();
    const originalGetGamepads = Object.getOwnPropertyDescriptor(
      Navigator.prototype,
      "getGamepads",
    );
    Object.defineProperty(navigator, "getGamepads", {
      configurable: true,
      value: () => [gamepad],
    });
    let engine: NavigationEngine | undefined;
    const captureEngine = (value: NavigationEngine): void => {
      engine = value;
    };
    const originalFocus = NavigationEngine.prototype.focus;
    const originalDispatch = NavigationEngine.prototype.dispatch;
    NavigationEngine.prototype.focus = function (
      focusId: Parameters<NavigationEngine["focus"]>[0],
      activateScope: Parameters<NavigationEngine["focus"]>[1] = true,
      options: Parameters<NavigationEngine["focus"]>[2],
    ): ReturnType<NavigationEngine["focus"]> {
      captureEngine(this);
      return originalFocus.call(this, focusId, activateScope, options);
    };
    NavigationEngine.prototype.dispatch = function (
      action: Parameters<NavigationEngine["dispatch"]>[0],
      inputSource: Parameters<NavigationEngine["dispatch"]>[1] = "programmatic",
    ): ReturnType<NavigationEngine["dispatch"]> {
      captureEngine(this);
      return originalDispatch.call(this, action, inputSource);
    };
    useProductStore.setState({
      activeView: "home",
      selectedGameId: null,
      returnView: "home",
      returnFocusId: null,
    });
    useLibraryStore.getState().reset();
    useNavigationStore.setState({
      activeScopeId: null,
      activeFocusId: null,
      previousFocusId: null,
      lastNavigationAction: null,
    });
    navigationRuntimeTrace.clear();
    const host = document.createElement("div");
    document.body.appendChild(host);
    const root = createRoot(host);
    const pressButton = async (buttonIndex: number): Promise<void> => {
      setTrigger(gamepad, buttonIndex, 1);
      await act(async () => waitForGamepadPoll());
      setTrigger(gamepad, buttonIndex, 0);
      await act(async () => waitForGamepadPoll());
    };
    const waitForInputValue = async (value: string): Promise<void> => {
      for (let attempt = 0; attempt < 100; attempt += 1) {
        const input = host.querySelector<HTMLInputElement>(
          '[data-gamepad-text-input="true"]',
        );
        if (input?.value === value) return;
        await new Promise((resolve) => setTimeout(resolve, 10));
      }
      throw new Error("The committed Library query did not materialize");
    };
    const latestContent = () =>
      navigationRuntimeTrace
        .getRecords()
        .filter((record) => record.event === "library_content_change")
        .slice(-1)[0];

    try {
      await act(async () => {
        root.render(
          <StrictMode>
            <App />
          </StrictMode>,
        );
      });
      await act(async () => useProductStore.getState().setView("library"));
      await act(async () => waitForSelector(host, "#library-heading"));
      await act(async () =>
        waitForSelector(host, '[data-focus-id="library-game-001"]'),
      );
      expect(engine?.focus("library-search")).toBe(true);
      await pressButton(0);
      await act(async () =>
        waitForSelector(host, '[data-keyboard-modal="true"]'),
      );
      for (const keyId of Array.from("juniper")) {
        expect(engine?.focus(`virtual-key-${keyId}`)).toBe(true);
        await pressButton(0);
      }
      await pressButton(7);
      await act(async () => waitForInputValue("juniper"));
      expect(useLibraryStore.getState()).toMatchObject({
        query: "juniper",
        queryCommitted: true,
      });
      await act(async () =>
        waitForSelector(host, '[data-focus-id^="library-game-"]'),
      );
      await act(async () => waitForGamepadPoll());

      const beforeDetails = latestContent();
      const opener = Array.from(
        host.querySelectorAll<HTMLElement>('[data-focus-id^="library-game-"]'),
      ).find((card) => card.textContent?.includes("Juniper Signal 08"));
      expect(beforeDetails).toMatchObject({
        queryLength: "juniper".length,
        queryCommitted: true,
        filterIds: ["all"],
        sortId: "title",
      });
      expect(opener).not.toBeUndefined();
      const openerFocusId = opener?.dataset.focusId;
      expect(openerFocusId).toBeDefined();
      expect(beforeDetails?.visibleResultIds).toContain(
        openerFocusId?.replace("library-", ""),
      );
      expect(engine?.focus(openerFocusId ?? "")).toBe(true);
      await pressButton(0);
      await act(async () => waitForSelector(host, "#details-heading"));
      await pressButton(1);
      await act(async () => waitForSelector(host, "#library-heading"));
      await act(async () => waitForGamepadPoll());

      const afterBack = latestContent();
      expect(afterBack).toBeDefined();
      expect(
        afterBack?.queryLength,
        `before=${JSON.stringify({
          queryVersion: beforeDetails?.queryVersion,
          queryLength: beforeDetails?.queryLength,
          queryCommitted: beforeDetails?.queryCommitted,
          filterIds: beforeDetails?.filterIds,
          sortId: beforeDetails?.sortId,
          resultCount: beforeDetails?.resultCount,
          visibleResultIds: beforeDetails?.visibleResultIds,
          resultGeneration: beforeDetails?.resultGeneration,
        })}; after=${JSON.stringify({
          queryVersion: afterBack?.queryVersion,
          queryLength: afterBack?.queryLength,
          queryCommitted: afterBack?.queryCommitted,
          filterIds: afterBack?.filterIds,
          sortId: afterBack?.sortId,
          resultCount: afterBack?.resultCount,
          visibleResultIds: afterBack?.visibleResultIds,
          resultGeneration: afterBack?.resultGeneration,
        })}`,
      ).toBe(beforeDetails?.queryLength);
      expect(afterBack?.queryCommitted).toBe(true);
      expect(afterBack?.filterIds).toEqual(beforeDetails?.filterIds);
      expect(afterBack?.sortId).toBe(beforeDetails?.sortId);
      expect(afterBack?.resultCount).toBe(beforeDetails?.resultCount);
      expect(afterBack?.resultGeneration).toBeGreaterThan(0);
      expect(afterBack?.visibleResultIds).toEqual(
        beforeDetails?.visibleResultIds,
      );
      expect(afterBack?.visibleResultIds).toContain(
        openerFocusId?.replace("library-", ""),
      );
      expect(afterBack?.openerPresentInResults).toBe(true);
    } finally {
      await act(async () => root.unmount());
      NavigationEngine.prototype.focus = originalFocus;
      NavigationEngine.prototype.dispatch = originalDispatch;
      if (originalGetGamepads) {
        Object.defineProperty(
          Navigator.prototype,
          "getGamepads",
          originalGetGamepads,
        );
      } else {
        Object.defineProperty(navigator, "getGamepads", {
          configurable: true,
          value: undefined,
        });
      }
    }
  });

  it("does not commit a cancelled VirtualKeyboard draft", async () => {
    const gamepad = makeTriggerGamepad();
    const originalGetGamepads = Object.getOwnPropertyDescriptor(
      Navigator.prototype,
      "getGamepads",
    );
    Object.defineProperty(navigator, "getGamepads", {
      configurable: true,
      value: () => [gamepad],
    });
    const { host, root } = await renderProductApp();

    const pressButton = async (buttonIndex: number): Promise<void> => {
      setTrigger(gamepad, buttonIndex, 1);
      await act(async () => waitForGamepadPoll());
      setTrigger(gamepad, buttonIndex, 0);
      await act(async () => waitForGamepadPoll());
    };

    try {
      await act(async () => useProductStore.getState().setView("library"));
      await act(async () => waitForSelector(host, "#library-heading"));
      const input = host.querySelector<HTMLInputElement>(
        '[data-focus-id="library-search"]',
      );
      expect(input).not.toBeNull();
      await act(async () => input?.click());
      await act(async () =>
        waitForSelector(host, '[data-keyboard-modal="true"]'),
      );
      await act(async () => dispatchKey("j"));
      await act(async () => dispatchKey("u"));
      await act(async () => dispatchKey("n"));
      expect(host.querySelector("output")?.textContent).toBe("jun");

      await pressButton(1);
      await act(async () => waitForSelector(host, "#library-heading"));
      expect(useLibraryStore.getState()).toMatchObject({
        query: "",
        queryVersion: 0,
        queryCommitted: false,
      });
    } finally {
      await act(async () => root.unmount());
      if (originalGetGamepads) {
        Object.defineProperty(
          Navigator.prototype,
          "getGamepads",
          originalGetGamepads,
        );
      } else {
        Object.defineProperty(navigator, "getGamepads", {
          configurable: true,
          value: undefined,
        });
      }
    }
  });

  it("preserves combined Library criteria through Details and Home remounts", async () => {
    const { host, root } = await renderProductApp();

    try {
      await act(async () => {
        useLibraryStore.getState().setQuery("juniper");
        useLibraryStore.getState().setStatus("completed");
        useLibraryStore.getState().setSort("recent");
        useProductStore.getState().setView("library");
      });
      await act(async () => waitForSelector(host, "#library-heading"));
      const beforeCard = host.querySelector<HTMLElement>(
        '[data-focus-id^="library-game-"]',
      );
      const beforeCount = host.querySelector(".page-hint")?.textContent;
      expect(beforeCard).not.toBeNull();
      expect(beforeCount).not.toBe("200 games match");

      await act(async () => beforeCard?.click());
      await act(async () => waitForSelector(host, "#details-heading"));
      // The gamepad Back contract is covered by the dedicated navigation tests;
      // this assertion isolates content continuity across the route remount.
      await act(async () => useProductStore.getState().closeDetails());
      await act(async () => waitForSelector(host, "#library-heading"));

      expect(useLibraryStore.getState()).toMatchObject({
        query: "juniper",
        status: "completed",
        sort: "recent",
        queryCommitted: true,
      });
      expect(host.querySelector(".page-hint")?.textContent).toBe(beforeCount);
      expect(
        host.querySelector(`[data-focus-id="${beforeCard?.dataset.focusId}"]`),
      ).not.toBeNull();

      await act(async () => useProductStore.getState().setView("home"));
      await act(async () => waitForSelector(host, "#home-heading"));
      await act(async () => useProductStore.getState().setView("library"));
      await act(async () => waitForSelector(host, "#library-heading"));
      expect(host.querySelector(".page-hint")?.textContent).toBe(beforeCount);
    } finally {
      await act(async () => root.unmount());
    }
  });

  it("resets Library criteria through the explicit clear action", async () => {
    const { host, root } = await renderProductApp();

    try {
      await act(async () => {
        useLibraryStore.getState().setQuery("juniper");
        useLibraryStore.getState().setStatus("completed");
        useLibraryStore.getState().setSort("time");
        useProductStore.getState().setView("library");
      });
      await act(async () => waitForSelector(host, "#library-heading"));
      await act(async () =>
        host
          .querySelector<HTMLElement>('[data-focus-id="library-clear-filters"]')
          ?.click(),
      );

      expect(useLibraryStore.getState()).toMatchObject({
        query: "",
        status: "all",
        sort: "title",
        queryCommitted: false,
      });
      expect(host.querySelector(".page-hint")?.textContent).toBe(
        "200 games match",
      );
    } finally {
      await act(async () => root.unmount());
    }
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

  it.each([0, 1, 3])(
    "activates delayed Details after a pending Library request (%i frames)",
    async (delayFrames) => {
      let engine: NavigationEngine | undefined;
      const host = document.createElement("div");
      document.body.appendChild(host);
      const root = createRoot(host);
      const releaseDetails: { current: (() => void) | null } = {
        current: null,
      };
      await act(async () => {
        root.render(
          <NavigationProvider>
            <PendingDetailsHarness
              delayFrames={delayFrames}
              onEngine={(value) => {
                engine = value;
              }}
              releaseDetails={releaseDetails}
            />
          </NavigationProvider>,
        );
      });
      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 0));
      });

      expect(engine).toBeDefined();
      expect(engine?.dispatch("move-down")).toBe(true);
      await act(async () => {
        host
          .querySelector<HTMLElement>('[data-focus-id="library-game-0"]')
          ?.click();
      });
      if (delayFrames > 0) {
        await act(async () => {
          for (let frame = 0; frame < delayFrames; frame += 1) {
            await new Promise((resolve) => setTimeout(resolve, 0));
          }
        });
      }
      await act(async () => releaseDetails.current?.());

      await act(async () =>
        waitForSelector(host, '[data-focus-id="details-play"]'),
      );

      const state = useNavigationStore.getState();
      expect(state.activeScopeId).toBe("details");
      expect(state.activeFocusId).toBe("details-play");
      expect(state.debug.pendingFocusId).toBeUndefined();
      expect(state.debug.canceledLibraryRequestId).toBeDefined();
      expect(document.activeElement?.getAttribute("data-focus-id")).toBe(
        "details-play",
      );
      expect(
        host
          .querySelector('[data-focus-id="details-play"]')
          ?.getAttribute("data-active"),
      ).toBe("true");

      await act(async () => root.unmount());
    },
  );

  it("keeps a valid Details focus across 50 open and close cycles", async () => {
    let engine: NavigationEngine | undefined;
    const host = document.createElement("div");
    document.body.appendChild(host);
    const root = createRoot(host);
    const releaseDetails: { current: (() => void) | null } = { current: null };
    await act(async () => {
      root.render(
        <NavigationProvider>
          <PendingDetailsHarness
            delayFrames={0}
            onEngine={(value) => {
              engine = value;
            }}
            releaseDetails={releaseDetails}
          />
        </NavigationProvider>,
      );
    });
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    for (let cycle = 0; cycle < 50; cycle += 1) {
      if (cycle > 0) {
        await act(async () => {
          await new Promise((resolve) => setTimeout(resolve, 60));
        });
      }
      expect(engine?.dispatch("move-down")).toBe(true);
      await act(async () => {
        host
          .querySelector<HTMLElement>('[data-focus-id="library-game-0"]')
          ?.click();
      });
      await act(async () => releaseDetails.current?.());
      await act(async () =>
        waitForSelector(host, '[data-focus-id="details-play"]'),
      );
      expect(useNavigationStore.getState().activeScopeId).toBe("details");
      expect(useNavigationStore.getState().activeFocusId).toBe("details-play");

      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 60));
      });
      await act(async () => {
        host
          .querySelector<HTMLElement>('[data-focus-id="details-back"]')
          ?.click();
      });
      await act(async () =>
        waitForSelector(host, '[data-focus-id="library-game-0"]'),
      );
      expect(useNavigationStore.getState().activeScopeId).toBe("product-shell");
      expect(useNavigationStore.getState().activeFocusId).toBe(
        "library-game-0",
      );
    }

    await act(async () => root.unmount());
  }, 15000);
});
