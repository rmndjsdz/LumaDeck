import { create } from "zustand";

import type {
  InputMode,
  NavigationAction,
  NavigationDebugState,
  NavigationPhase,
} from "../ui/navigation/core/navigation-types";

const initialDebug: NavigationDebugState = {
  registryCount: 0,
  evaluatedCandidates: [],
  resolutionTimeMs: 0,
  gamepadConnected: false,
  actionsPerSecond: 0,
  focusLosses: 0,
  duplicateFocusIds: [],
  navigationPhase: "idle",
};

let actionTimes: number[] = [];

interface NavigationState {
  inputMode: InputMode;
  activeScopeId: string | null;
  activeFocusId: string | null;
  previousFocusId: string | null;
  lastNavigationAction: NavigationAction | null;
  navigationPhase: NavigationPhase;
  debug: NavigationDebugState;
  setInputMode: (inputMode: InputMode) => void;
  setActiveScopeId: (scopeId: string | null) => void;
  setActiveFocusId: (focusId: string | null) => void;
  recordAction: (action: NavigationAction) => void;
  setNavigationPhase: (phase: NavigationPhase) => void;
  updateDebug: (debug: Partial<NavigationDebugState>) => void;
  setGamepadConnected: (connected: boolean) => void;
  setGamepadDiagnostic: (gamepad?: NavigationDebugState["gamepad"]) => void;
}

export const useNavigationStore = create<NavigationState>((set) => ({
  inputMode: "mouse",
  activeScopeId: null,
  activeFocusId: null,
  previousFocusId: null,
  lastNavigationAction: null,
  navigationPhase: "idle",
  debug: initialDebug,
  setInputMode: (inputMode) =>
    set((state) => (state.inputMode === inputMode ? state : { inputMode })),
  setActiveScopeId: (activeScopeId) => set({ activeScopeId }),
  setActiveFocusId: (activeFocusId) =>
    set((state) => ({
      previousFocusId:
        activeFocusId && activeFocusId !== state.activeFocusId
          ? state.activeFocusId
          : state.previousFocusId,
      activeFocusId,
      debug: {
        ...state.debug,
        focusLosses:
          activeFocusId === null &&
          state.activeFocusId !== null &&
          state.activeScopeId !== null
            ? state.debug.focusLosses + 1
            : state.debug.focusLosses,
      },
    })),
  recordAction: (lastNavigationAction) =>
    set((state) => {
      const now = performance.now();
      actionTimes = [...actionTimes.filter((time) => now - time < 1000), now];
      return {
        lastNavigationAction,
        debug: {
          ...state.debug,
          actionsPerSecond: actionTimes.length,
        },
      };
    }),
  setNavigationPhase: (navigationPhase) =>
    set((state) =>
      state.navigationPhase === navigationPhase
        ? state
        : {
            navigationPhase,
            debug: { ...state.debug, navigationPhase },
          },
    ),
  updateDebug: (debug) =>
    set((state) => ({ debug: { ...state.debug, ...debug } })),
  setGamepadConnected: (gamepadConnected) =>
    set((state) => ({
      debug: { ...state.debug, gamepadConnected },
    })),
  setGamepadDiagnostic: (gamepad) =>
    set((state) => ({
      debug: { ...state.debug, gamepad },
    })),
}));
