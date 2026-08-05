import { act, useEffect, useState, type ComponentProps } from "react";
import { createRoot, type Root } from "react-dom/client";
import { describe, expect, it } from "vitest";

import { useNavigationStore } from "../../../stores/navigation-store";
import { FocusScope } from "../focus/FocusScope";
import { Focusable } from "../focus/Focusable";
import { NavigationProvider } from "../NavigationProvider";
import { useNavigation } from "../navigation-context";
import { NavigationTab, NavigationTabs } from "./NavigationTabs";
import type { NavigationEngine } from "../core/navigation-engine";
import type { NavigationAction } from "../core/navigation-types";

interface MutableGamepad extends Omit<Gamepad, "axes" | "buttons"> {
  axes: number[];
  buttons: Array<{
    pressed: boolean;
    touched: boolean;
    value: number;
  }>;
}

function makeGamepad(): MutableGamepad {
  return {
    axes: [0, 0, 0, 0, 0, 0],
    buttons: Array.from({ length: 8 }, () => ({
      pressed: false,
      touched: false,
      value: 0,
    })),
    connected: true,
    id: "navigation-tabs-test-pad",
    index: 0,
    mapping: "standard",
    timestamp: 0,
    vibrationActuator: {
      playEffect: async () => "complete",
      reset: async () => "complete",
    },
  };
}

function DetailsTabsHarness({
  activationMode = "automatic",
  gameId = "game-001",
  engineRef,
}: {
  activationMode?: "automatic" | "manual";
  gameId?: string;
  engineRef?: { current: NavigationEngine | null };
}) {
  const [selectedId, setSelectedId] = useState("details-tab-summary");
  const [transitionDirection, setTransitionDirection] = useState<
    "forward" | "backward"
  >("forward");
  const { engine } = useNavigation();

  if (engineRef) engineRef.current = engine;

  const selectTab = (focusId: string) => {
    if (focusId === selectedId) return;
    setTransitionDirection(
      selectedId === "details-tab-summary" && focusId === "details-tab-activity"
        ? "forward"
        : "backward",
    );
    setSelectedId(focusId);
  };

  const handleAction = (action: NavigationAction): boolean => {
    if (action !== "page-next" && action !== "page-previous") return false;
    const navigableTabs = [
      "details-tab-summary",
      "details-tab-activity",
    ] as const;
    const currentIndex = navigableTabs.indexOf(
      selectedId as (typeof navigableTabs)[number],
    );
    const nextTab =
      navigableTabs[currentIndex + (action === "page-next" ? 1 : -1)];
    if (!nextTab) return true;
    selectTab(nextTab);
    engine.focus(nextTab);
    return true;
  };

  useEffect(() => {
    setSelectedId("details-tab-summary");
    setTransitionDirection("forward");
  }, [gameId]);

  return (
    <FocusScope
      scopeId="details"
      initialFocusId="details-play"
      trapFocus
      activateOnMount
      onAction={handleAction}
    >
      <Focusable
        focusId="details-play"
        scopeId="details"
        navigation={{ down: selectedId }}
      >
        Play
      </Focusable>
      <NavigationTabs
        groupId="details-sections"
        selectedId={selectedId}
        onSelect={selectTab}
        activationMode={activationMode}
        upTargetId="details-play"
        ariaLabel="Game sections"
      >
        <NavigationTab focusId="details-tab-summary" scopeId="details">
          Summary
        </NavigationTab>
        <NavigationTab focusId="details-tab-activity" scopeId="details">
          Activity
        </NavigationTab>
        <NavigationTab
          focusId="details-tab-disabled"
          scopeId="details"
          disabled
        >
          Disabled
        </NavigationTab>
      </NavigationTabs>
      <output data-testid="transition-direction">{transitionDirection}</output>
      <output data-testid="selected-tab">{selectedId}</output>
      <output data-testid="game-id">{gameId}</output>
    </FocusScope>
  );
}

async function flushEffects(): Promise<void> {
  await act(async () => {
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
  });
}

function dispatchKey(key: string): void {
  window.dispatchEvent(
    new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }),
  );
  window.dispatchEvent(new KeyboardEvent("keyup", { key, bubbles: true }));
}

function renderTabs(props: ComponentProps<typeof DetailsTabsHarness> = {}): {
  root: Root;
  host: HTMLDivElement;
  engineRef: { current: NavigationEngine | null };
} {
  useNavigationStore.setState({
    activeScopeId: null,
    activeFocusId: null,
    previousFocusId: null,
    lastNavigationAction: null,
  });
  const host = document.createElement("div");
  document.body.appendChild(host);
  const root = createRoot(host);
  const engineRef = { current: null as NavigationEngine | null };
  act(() =>
    root.render(
      <NavigationProvider>
        <DetailsTabsHarness {...props} engineRef={engineRef} />
      </NavigationProvider>,
    ),
  );
  return { root, host, engineRef };
}

function cleanup(root: Root, host: HTMLDivElement): void {
  act(() => root.unmount());
  host.remove();
}

describe("NavigationTabs", () => {
  it("automatically selects tabs while moving with the keyboard", async () => {
    const { root, host } = renderTabs();
    await flushEffects();

    expect(useNavigationStore.getState().activeFocusId).toBe("details-play");
    act(() => dispatchKey("ArrowDown"));
    expect(useNavigationStore.getState().activeFocusId).toBe(
      "details-tab-summary",
    );
    expect(host.querySelector('[aria-selected="true"]')?.textContent).toBe(
      "Summary",
    );

    act(() => dispatchKey("ArrowRight"));
    expect(useNavigationStore.getState().activeFocusId).toBe(
      "details-tab-activity",
    );
    expect(
      host.querySelector('[data-testid="selected-tab"]')?.textContent,
    ).toBe("details-tab-activity");

    act(() => dispatchKey("ArrowLeft"));
    expect(useNavigationStore.getState().activeFocusId).toBe(
      "details-tab-summary",
    );
    cleanup(root, host);
  });

  it("resolves Up to the declared target and Down to the selected tab", async () => {
    const { root, host } = renderTabs();
    await flushEffects();

    act(() => dispatchKey("ArrowDown"));
    act(() => dispatchKey("ArrowRight"));
    act(() => dispatchKey("ArrowUp"));
    expect(useNavigationStore.getState().activeFocusId).toBe("details-play");

    act(() => dispatchKey("ArrowDown"));
    expect(useNavigationStore.getState().activeFocusId).toBe(
      "details-tab-activity",
    );
    cleanup(root, host);
  });

  it("keeps manual activation until confirm", async () => {
    const { root, host } = renderTabs({ activationMode: "manual" });
    await flushEffects();

    act(() => dispatchKey("ArrowDown"));
    act(() => dispatchKey("ArrowRight"));
    expect(
      host.querySelector('[data-testid="selected-tab"]')?.textContent,
    ).toBe("details-tab-summary");
    expect(useNavigationStore.getState().activeFocusId).toBe(
      "details-tab-activity",
    );
    act(() => dispatchKey("Enter"));
    expect(
      host.querySelector('[data-testid="selected-tab"]')?.textContent,
    ).toBe("details-tab-activity");
    cleanup(root, host);
  });

  it("uses page actions for synchronized focus and selection without wrapping", async () => {
    const { root, host, engineRef } = renderTabs();
    await flushEffects();

    act(() => {
      engineRef.current?.dispatch("page-next", "gamepad");
    });
    expect(useNavigationStore.getState().activeFocusId).toBe(
      "details-tab-activity",
    );
    expect(host.querySelector('[aria-selected="true"]')?.textContent).toBe(
      "Activity",
    );
    expect(
      host.querySelector('[data-testid="transition-direction"]')?.textContent,
    ).toBe("forward");

    act(() => {
      engineRef.current?.dispatch("page-next", "gamepad");
    });
    expect(useNavigationStore.getState().activeFocusId).toBe(
      "details-tab-activity",
    );
    expect(
      host.querySelector('[data-testid="selected-tab"]')?.textContent,
    ).toBe("details-tab-activity");

    act(() => {
      engineRef.current?.dispatch("page-previous", "gamepad");
    });
    expect(useNavigationStore.getState().activeFocusId).toBe(
      "details-tab-summary",
    );
    expect(
      host.querySelector('[data-testid="transition-direction"]')?.textContent,
    ).toBe("backward");

    act(() => {
      engineRef.current?.dispatch("page-previous", "gamepad");
    });
    expect(useNavigationStore.getState().activeFocusId).toBe(
      "details-tab-summary",
    );
    cleanup(root, host);
  });

  it("starts page navigation from Play and skips the disabled tab", async () => {
    const { root, host, engineRef } = renderTabs();
    await flushEffects();

    expect(useNavigationStore.getState().activeFocusId).toBe("details-play");
    act(() => {
      engineRef.current?.dispatch("page-next", "gamepad");
    });
    expect(useNavigationStore.getState().activeFocusId).toBe(
      "details-tab-activity",
    );
    expect(
      host.querySelector('[data-focus-id="details-tab-disabled"]'),
    ).not.toBeNull();
    expect(
      host
        .querySelector('[data-focus-id="details-tab-disabled"]')
        ?.getAttribute("aria-disabled"),
    ).toBe("true");
    cleanup(root, host);
  });

  it("skips disabled tabs and supports mouse activation", async () => {
    const { root, host } = renderTabs();
    await flushEffects();

    act(() => dispatchKey("ArrowDown"));
    act(() => dispatchKey("ArrowRight"));
    act(() => dispatchKey("ArrowRight"));
    expect(useNavigationStore.getState().activeFocusId).toBe(
      "details-tab-activity",
    );

    act(() => {
      host
        .querySelector<HTMLElement>('[data-focus-id="details-tab-summary"]')
        ?.click();
    });
    expect(useNavigationStore.getState().activeFocusId).toBe(
      "details-tab-summary",
    );
    expect(
      host.querySelector('[data-testid="selected-tab"]')?.textContent,
    ).toBe("details-tab-summary");
    cleanup(root, host);
  });

  it("supports the gamepad direction path and resets to Summary for a new game", async () => {
    const gamepad = makeGamepad();
    const original = Object.getOwnPropertyDescriptor(
      Navigator.prototype,
      "getGamepads",
    );
    Object.defineProperty(navigator, "getGamepads", {
      configurable: true,
      value: () => [gamepad],
    });
    const { root, host } = renderTabs();

    try {
      await flushEffects();
      gamepad.axes[1] = 1;
      await act(async () => new Promise((resolve) => setTimeout(resolve, 30)));
      gamepad.axes[1] = 0;
      await act(async () => new Promise((resolve) => setTimeout(resolve, 30)));
      gamepad.axes[0] = 1;
      await act(async () => new Promise((resolve) => setTimeout(resolve, 30)));
      expect(useNavigationStore.getState().activeFocusId).toBe(
        "details-tab-activity",
      );

      act(() =>
        root.render(
          <NavigationProvider>
            <DetailsTabsHarness gameId="game-002" />
          </NavigationProvider>,
        ),
      );
      await flushEffects();
      expect(
        host.querySelector('[data-testid="selected-tab"]')?.textContent,
      ).toBe("details-tab-summary");
    } finally {
      cleanup(root, host);
      if (original) {
        Object.defineProperty(Navigator.prototype, "getGamepads", original);
      } else {
        Object.defineProperty(Navigator.prototype, "getGamepads", {
          configurable: true,
          value: undefined,
        });
      }
    }
  });
});
