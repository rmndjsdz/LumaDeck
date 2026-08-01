import { create } from "zustand";

import type {
  InputMode,
  NavigationAction,
  NavigationDebugState,
} from "../ui/navigation/core/navigation-types";

const initialDebug: NavigationDebugState = {
  registryCount: 0,
  evaluatedCandidates: [],
  resolutionTimeMs: 0,
  gamepadConnected: false,
  actionsPerSecond: 0,
  focusLosses: 0,
  duplicateFocusIds: [],
};

let actionTimes: number[] = [];

interface NavigationState {
  inputMode: InputMode;
  activeScopeId: string | null;
  activeFocusId: string | null;
  previousFocusId: string | null;
  lastNavigationAction: NavigationAction | null;
  debug: NavigationDebugState;
  setInputMode: (inputMode: InputMode) => void;
  setActiveScopeId: (scopeId: string | null) => void;
  setActiveFocusId: (focusId: string | null) => void;
  recordAction: (action: NavigationAction) => void;
  updateDebug: (debug: Partial<NavigationDebugState>) => void;
  setGamepadConnected: (connected: boolean) => void;
}

export const useNavigationStore = create<NavigationState>((set) => ({
  inputMode: "mouse",
  activeScopeId: null,
  activeFocusId: null,
  previousFocusId: null,
  lastNavigationAction: null,
  debug: initialDebug,
  setInputMode: (inputMode) => set({ inputMode }),
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
  updateDebug: (debug) =>
    set((state) => ({ debug: { ...state.debug, ...debug } })),
  setGamepadConnected: (gamepadConnected) =>
    set((state) => ({
      debug: { ...state.debug, gamepadConnected },
    })),
}));
