import { act, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { describe, expect, it } from "vitest";
import App from "../App";
import { useNavigationStore } from "../stores/navigation-store";
import { useProductStore } from "../stores/product-store";
import { useNavigation } from "../ui/navigation/navigation-context";
import { FocusScope } from "../ui/navigation/focus/FocusScope";
import { Focusable } from "../ui/navigation/focus/Focusable";
import { NavigationProvider } from "../ui/navigation/NavigationProvider";
import { NavigationGrid } from "../ui/navigation/layouts/NavigationGrid";
import type { NavigationEngine } from "../ui/navigation/core/navigation-engine";

async function waitForCatalog(): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 0));
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
    root.render(<App />);
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
                onConfirm={() => setView("library")}
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
    await act(async () =>
      useProductStore
        .getState()
        .openDetails("game-001", "library", "library-game-001"),
    );
    expect(host.querySelector("#details-heading")).not.toBeNull();
    expect(
      host
        .querySelector('[data-focus-id="details-play"]')
        ?.getAttribute("data-active"),
    ).toBe("true");

    await act(async () => useProductStore.getState().closeDetails());
    expect(host.querySelector("#library-heading")).not.toBeNull();
    expect(
      host
        .querySelector('[data-focus-id="library-game-001"]')
        ?.getAttribute("data-active"),
    ).toBe("true");

    await act(async () => root.unmount());
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
