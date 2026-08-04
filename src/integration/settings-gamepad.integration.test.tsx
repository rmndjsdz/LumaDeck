import { act, StrictMode } from "react";
import { createRoot, type Root } from "react-dom/client";
import { describe, expect, it } from "vitest";
import App from "../App";
import { useNavigationStore } from "../stores/navigation-store";
import { useProductStore } from "../stores/product-store";
import { navigationRuntimeTrace } from "../ui/navigation/debug/navigation-runtime-trace";

interface MutablePad extends Omit<Gamepad, "axes" | "buttons"> {
  axes: number[];
  buttons: Array<{ pressed: boolean; touched: boolean; value: number }>;
}

function makePad(): MutablePad {
  return {
    axes: [0, 0, 0, 0, 0, 0],
    buttons: Array.from({ length: 16 }, () => ({
      pressed: false,
      touched: false,
      value: 0,
    })),
    connected: true,
    id: "settings-qa-pad",
    index: 0,
    mapping: "standard",
    timestamp: 0,
    vibrationActuator: {
      playEffect: async () => "complete",
      reset: async () => "complete",
    },
  };
}

async function waitFor(predicate: () => boolean): Promise<void> {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (predicate()) return;
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
  throw new Error("condition did not become true");
}

async function tick(): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 40));
}

function press(pad: MutablePad, index: number): void {
  const button = pad.buttons[index];
  if (!button) throw new Error(`missing button ${index}`);
  button.pressed = true;
  button.value = 1;
}

function release(pad: MutablePad, index: number): void {
  const button = pad.buttons[index];
  if (!button) throw new Error(`missing button ${index}`);
  button.pressed = false;
  button.value = 0;
}

async function tap(pad: MutablePad, index: number): Promise<void> {
  press(pad, index);
  await act(tick);
  release(pad, index);
  await act(tick);
}

async function renderWithPad(
  pad: MutablePad,
): Promise<{ host: HTMLElement; root: Root }> {
  Object.defineProperty(navigator, "getGamepads", {
    configurable: true,
    value: () => [pad],
  });
  navigationRuntimeTrace.clear();
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
    await tick();
  });
  return { host, root };
}

describe("Settings gamepad QA", () => {
  it("transitions Library → Settings, navigates, opens Integrations, and backs out", async () => {
    const pad = makePad();
    const { host, root } = await renderWithPad(pad);
    try {
      const homeInitialFocusId = useNavigationStore.getState().activeFocusId;
      await tap(pad, 13);
      const homeFirstMoveFocusId = useNavigationStore.getState().activeFocusId;
      const homeTrace = navigationRuntimeTrace.getRecords();

      await act(async () => {
        useProductStore.getState().setView("library");
        await tick();
      });
      await tap(pad, 7);
      await waitFor(() => useProductStore.getState().activeView === "settings");

      const settingsState = useNavigationStore.getState();
      const initialFocusId = settingsState.activeFocusId;
      expect(useProductStore.getState().activeView).toBe("settings");
      expect(settingsState.activeScopeId).toBe("settings-shell");
      expect(initialFocusId).not.toBeNull();
      expect(
        host.querySelector(`[data-focus-id="${initialFocusId}"]`),
      ).not.toBeNull();
      const settingsFocusableSnapshot = Array.from(
        host.querySelectorAll<HTMLElement>('[data-focusable="true"]'),
      ).map((element) => ({
        focusId: element.dataset.focusId ?? null,
        scopeId:
          element.closest<HTMLElement>("[data-focus-scope]")?.dataset
            .focusScope,
        disabled: element.getAttribute("aria-disabled") === "true",
        visible:
          element.isConnected && getComputedStyle(element).display !== "none",
        active: element.dataset.active === "true",
      }));

      const settingsActivationTrace = navigationRuntimeTrace.getRecords();
      const initialFocusCommits = settingsActivationTrace.filter(
        (record) =>
          record.route === "settings" &&
          record.event === "FOCUS_COMMIT" &&
          record.details.selectedFocusId === "settings-integrations",
      );

      await tap(pad, 13);
      const firstMoveFocusId = useNavigationStore.getState().activeFocusId;
      const movementChanged = firstMoveFocusId !== initialFocusId;

      // Storage is now an available settings screen; use Accessibility to preserve
      // the coming-soon feedback assertion without invoking migration UI.
      if (firstMoveFocusId === "settings-storage") await tap(pad, 13);
      const comingSoonFocusId = useNavigationStore.getState().activeFocusId;
      await tap(pad, 0);
      const comingSoonFeedbackAfterGamepad =
        host.querySelector('[data-availability-feedback="coming-soon"]') !==
        null;
      const gamepadComingSoonPreservedFocus =
        useNavigationStore.getState().activeFocusId === comingSoonFocusId &&
        useProductStore.getState().activeView === "settings";
      const dismissButton = host.querySelector<HTMLButtonElement>(
        '[data-availability-feedback="coming-soon"] button',
      );
      await act(async () => {
        dismissButton?.click();
        await tick();
      });
      const feedbackDismissedWithFocusPreserved =
        host.querySelector('[data-availability-feedback="coming-soon"]') ===
          null &&
        useNavigationStore.getState().activeFocusId === comingSoonFocusId;

      for (let index = 0; index < 5; index += 1) {
        await tap(pad, 12);
      }
      for (let index = 0; index < 7; index += 1) {
        await tap(pad, 13);
      }
      const allSettingsCardsTraversed = [
        "settings-general",
        "settings-appearance",
        "settings-navigation",
        "settings-library",
        "settings-integrations",
        "settings-storage",
        "settings-accessibility",
        "settings-information",
      ].every((focusId) =>
        settingsFocusableSnapshot.some((item) => item.focusId === focusId),
      );

      await tap(pad, 14);
      const afterLeftFocusId = useNavigationStore.getState().activeFocusId;
      await tap(pad, 15);
      const afterRightFocusId = useNavigationStore.getState().activeFocusId;
      const lateralNavigationKeptFocus =
        afterLeftFocusId !== null && afterRightFocusId !== null;

      const generalCard = host.querySelector<HTMLButtonElement>(
        '[data-focus-id="settings-general"]',
      );
      await act(async () => {
        generalCard?.click();
        await tick();
      });
      const mouseComingSoonFeedback =
        host.querySelector('[data-availability-feedback="coming-soon"]') !==
        null;
      const mousePreservedFocus =
        useNavigationStore.getState().activeFocusId === "settings-general";
      const mouseDismissButton = host.querySelector<HTMLButtonElement>(
        '[data-availability-feedback="coming-soon"] button',
      );
      await act(async () => {
        mouseDismissButton?.click();
        await tick();
      });

      for (let index = 0; index < 8; index += 1) {
        if (
          useNavigationStore.getState().activeFocusId ===
          "settings-integrations"
        ) {
          break;
        }
        await tap(pad, 13);
      }
      const reachedIntegrations =
        useNavigationStore.getState().activeFocusId === "settings-integrations";
      expect(reachedIntegrations).toBe(true);
      await tap(pad, 0);
      const openedIntegrations =
        host
          .querySelector("#settings-heading")
          ?.textContent?.includes("Integraciones") ?? false;

      await tap(pad, 1);
      await act(tick);
      const backToSettings =
        useProductStore.getState().activeView === "settings" &&
        (host
          .querySelector("#settings-heading")
          ?.textContent?.includes("Configuración") ??
          false);

      await tap(pad, 1);
      const backToLibrary = useProductStore.getState().activeView === "library";

      const trace = navigationRuntimeTrace.getRecords();
      const registeredFocusIds = new Set(
        trace
          .filter((record) => record.event === "registerFocusable")
          .map((record) => record.details.focusId)
          .filter((focusId): focusId is string => typeof focusId === "string"),
      );
      console.log(
        JSON.stringify(
          {
            state: {
              activeView: useProductStore.getState().activeView,
              activeScopeId: useNavigationStore.getState().activeScopeId,
              activeFocusId: useNavigationStore.getState().activeFocusId,
              documentActiveFocusId:
                document.activeElement?.getAttribute("data-focus-id"),
              movementChanged,
              initialFocusCommits: initialFocusCommits.length,
              firstMoveFocusId,
              allSettingsCardsTraversed,
              lateralNavigationKeptFocus,
              comingSoonFeedbackAfterGamepad,
              gamepadComingSoonPreservedFocus,
              feedbackDismissedWithFocusPreserved,
              mouseComingSoonFeedback,
              mousePreservedFocus,
              openedIntegrations,
              backToSettings,
              backToLibrary,
            },
            homeComparison: {
              scopeId: "product-shell",
              initialFocusId: homeInitialFocusId,
              firstMoveFocusId: homeFirstMoveFocusId,
              firstNavInput: homeTrace.find(
                (record) =>
                  record.event === "NAV_INPUT" && record.details.direction,
              ),
              firstNavResolve: homeTrace.find(
                (record) => record.event === "NAV_RESOLVE",
              ),
            },
            focusables: settingsFocusableSnapshot,
            trace: trace
              .filter((record) =>
                [
                  "PRIMARY_SCREEN_TRANSITION_REQUEST",
                  "route_transition",
                  "scope_register",
                  "scope_active",
                  "FOCUS_COMMIT",
                  "NAV_INPUT",
                  "NAV_RESOLVE",
                  "NAV_INPUT_BLOCKED",
                ].includes(record.event),
              )
              .map((record) => ({
                event: record.event,
                route: record.route,
                activeScopeId: record.activeScopeId,
                activeFocusId: record.activeFocusId,
                domActiveElementFocusId: record.domActiveElementFocusId,
                focusableRegistered: record.focusableRegistered,
                regionId: record.regionId,
                details: record.details,
              })),
          },
          null,
          2,
        ),
      );

      expect(initialFocusId && registeredFocusIds.has(initialFocusId)).toBe(
        true,
      );
      expect(initialFocusCommits).toHaveLength(1);
      expect(movementChanged).toBe(true);
      expect(firstMoveFocusId).toBe("settings-storage");
      expect(allSettingsCardsTraversed).toBe(true);
      expect(lateralNavigationKeptFocus).toBe(true);
      expect(comingSoonFeedbackAfterGamepad).toBe(true);
      expect(gamepadComingSoonPreservedFocus).toBe(true);
      expect(feedbackDismissedWithFocusPreserved).toBe(true);
      expect(mouseComingSoonFeedback).toBe(true);
      expect(mousePreservedFocus).toBe(true);
      expect(openedIntegrations).toBe(true);
      expect(backToSettings).toBe(true);
      expect(backToLibrary).toBe(true);
    } finally {
      await act(async () => root.unmount());
      host.remove();
    }
  }, 15000);
});
