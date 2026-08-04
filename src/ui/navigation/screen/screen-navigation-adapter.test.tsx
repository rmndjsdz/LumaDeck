import { StrictMode, act, useRef, useState, type ReactNode } from "react";
import { createRoot, type Root } from "react-dom/client";
import { describe, expect, it } from "vitest";

import { useNavigationStore } from "../../../stores/navigation-store";
import { FocusScope } from "../focus/FocusScope";
import { Focusable } from "../focus/Focusable";
import { NavigationProvider } from "../NavigationProvider";
import { useNavigation } from "../navigation-context";
import { ScreenNavigationAdapter } from "./ScreenNavigationAdapter";
import type { NavigationScreenDefinition } from "./navigation-screen-contract";

const definition: NavigationScreenDefinition = {
  id: "test-screen",
  route: "test",
  rootScope: { scopeId: "test-root" },
  initialFocus: "test-initial",
  regions: [],
  rowGroups: [],
  restorePolicy: { restoreFocus: true, rememberScroll: true },
};

function resetNavigationState(): void {
  useNavigationStore.setState({
    activeScopeId: null,
    activeFocusId: null,
    previousFocusId: null,
    lastNavigationAction: null,
  });
}

async function flushEffects(): Promise<void> {
  await act(async () => {
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
  });
}

function TestScreen() {
  return (
    <ScreenNavigationAdapter definition={definition}>
      <Focusable focusId="test-initial" scopeId="test-root">
        Initial
      </Focusable>
      <Focusable focusId="test-secondary" scopeId="test-root">
        Secondary
      </Focusable>
    </ScreenNavigationAdapter>
  );
}

function RestorableScreen({
  engineRef,
}: {
  engineRef: { current: ReturnType<typeof useNavigation>["engine"] | null };
}) {
  const { engine } = useNavigation();
  const [detailsOpen, setDetailsOpen] = useState(false);
  const transitionIdRef = useRef(0);
  engineRef.current = engine;

  const openDetails = () => {
    engine.prepareScopeOpen("legacy-details", "test-secondary");
    setDetailsOpen(true);
  };
  const closeDetails = () => {
    transitionIdRef.current += 1;
    engine.requestScopeRestore(
      "legacy-details",
      "test-root",
      `test-transition-${transitionIdRef.current}`,
    );
    setDetailsOpen(false);
  };

  return (
    <ScreenNavigationAdapter definition={definition}>
      {!detailsOpen ? (
        <>
          <Focusable focusId="test-initial" scopeId="test-root">
            Initial
          </Focusable>
          <Focusable
            focusId="test-secondary"
            scopeId="test-root"
            onConfirm={openDetails}
          >
            Open Details
          </Focusable>
        </>
      ) : (
        <FocusScope
          scopeId="legacy-details"
          parentScopeId="test-root"
          initialFocusId="legacy-action"
          restoreFocus
          modal
          trapFocus
          activateOnMount
        >
          <Focusable
            focusId="legacy-action"
            scopeId="legacy-details"
            onConfirm={closeDetails}
          >
            Close
          </Focusable>
        </FocusScope>
      )}
    </ScreenNavigationAdapter>
  );
}

function renderApp(
  children: ReactNode,
  strict = false,
): {
  root: Root;
  host: HTMLDivElement;
} {
  resetNavigationState();
  const host = document.createElement("div");
  document.body.appendChild(host);
  const root = createRoot(host);
  const content = <NavigationProvider>{children}</NavigationProvider>;
  act(() => root.render(strict ? <StrictMode>{content}</StrictMode> : content));
  return { root, host };
}

describe("ScreenNavigationAdapter", () => {
  it("registers the screen definition and activates its root scope", async () => {
    const { root, host } = renderApp(<TestScreen />);
    await flushEffects();

    expect(useNavigationStore.getState().activeScopeId).toBe("test-root");
    expect(useNavigationStore.getState().activeFocusId).toBe("test-initial");
    expect(host.querySelector('[data-focus-id="test-initial"]')).not.toBeNull();

    act(() => root.unmount());
    host.remove();
  });

  it("keeps initial focus as the existing engine fallback", async () => {
    const { root, host } = renderApp(<TestScreen />, true);
    await flushEffects();

    expect(document.activeElement?.getAttribute("data-focus-id")).toBe(
      "test-initial",
    );
    expect(useNavigationStore.getState().activeFocusId).toBe("test-initial");

    act(() => root.unmount());
    host.remove();
  });

  it("restores the exact context once while a legacy Details scope coexists", async () => {
    const engineRef: {
      current: ReturnType<typeof useNavigation>["engine"] | null;
    } = { current: null };
    const { root, host } = renderApp(
      <RestorableScreen engineRef={engineRef} />,
    );
    await flushEffects();

    let focusedSecondary = false;
    act(() => {
      focusedSecondary = engineRef.current?.focus("test-secondary") ?? false;
    });
    expect(focusedSecondary).toBe(true);
    await act(async () => {
      host
        .querySelector<HTMLElement>('[data-focus-id="test-secondary"]')
        ?.click();
      await new Promise<void>((resolve) => setTimeout(resolve, 0));
    });
    expect(useNavigationStore.getState().activeScopeId).toBe("legacy-details");

    await act(async () => {
      await new Promise<void>((resolve) => setTimeout(resolve, 60));
      host
        .querySelector<HTMLElement>('[data-focus-id="legacy-action"]')
        ?.click();
      await new Promise<void>((resolve) => setTimeout(resolve, 0));
    });

    expect(useNavigationStore.getState().activeScopeId).toBe("test-root");
    expect(useNavigationStore.getState().activeFocusId).toBe("test-secondary");
    const restoreCommits =
      engineRef.current
        ?.getNavigationTrace()
        .filter((record) => record.event === "CONTEXT_RESTORE_COMMIT") ?? [];
    expect(restoreCommits).toHaveLength(1);
    expect(restoreCommits[0]?.selectedFocusId).toBe("test-secondary");

    act(() => root.unmount());
    host.remove();
  });

  it("does not leave an active scope after adapter unmount", async () => {
    const { root, host } = renderApp(<TestScreen />);
    await flushEffects();

    act(() => root.unmount());

    expect(useNavigationStore.getState().activeScopeId).toBeNull();
    expect(useNavigationStore.getState().activeFocusId).toBeNull();
    host.remove();
  });
});
