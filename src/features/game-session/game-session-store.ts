import { create } from "zustand";
import type {
  GameSessionState,
  GameSessionStatus,
  MonitoringMode,
  SessionCapabilities,
} from "./game-session-types";

interface GameSessionStore {
  currentState: GameSessionState;
  sessionId: string;
  gameId: string;
  steamAppId: number;
  source: string;
  title: string;
  artwork: string;
  elapsedSeconds: number;
  message: string;
  unsupportedReason: string | null;
  monitoringMode: MonitoringMode;
  antiCheatProvider: string | null;
  compatibleReason: string | null;
  capabilities: SessionCapabilities;
  inputFrozen: boolean;
  returnFocusId: string | null;
  applyStatus: (status: GameSessionStatus) => void;
  setGamePresentation: (title: string, artwork: string) => void;
  setReturnFocusId: (focusId: string | null) => void;
  clearReturnFocusId: () => void;
}

const isInputFrozenState = (state: GameSessionState): boolean =>
  state === "preparing" ||
  state === "launching" ||
  state === "running" ||
  state === "finishing";

export const useGameSessionStore = create<GameSessionStore>((set) => ({
  currentState: "idle",
  sessionId: "",
  gameId: "",
  steamAppId: 0,
  source: "none",
  title: "",
  artwork: "",
  elapsedSeconds: 0,
  message: "",
  unsupportedReason: null,
  monitoringMode: "full",
  antiCheatProvider: null,
  compatibleReason: null,
  capabilities: {
    playtime: true,
    startTime: true,
    endTime: true,
    processTracking: true,
    advancedProcessMetrics: true,
  },
  inputFrozen: false,
  returnFocusId: null,
  applyStatus: (status) =>
    set({
      currentState: status.state,
      sessionId: status.sessionId,
      gameId: status.gameId,
      steamAppId: status.steamAppId,
      source: status.source,
      elapsedSeconds: status.elapsedSeconds,
      message: status.message,
      unsupportedReason: status.unsupportedReason ?? null,
      monitoringMode: status.monitoringMode,
      antiCheatProvider: status.antiCheatProvider ?? null,
      compatibleReason: status.compatibleReason ?? null,
      capabilities: status.capabilities,
      inputFrozen: isInputFrozenState(status.state),
    }),
  setGamePresentation: (title, artwork) => set({ title, artwork }),
  setReturnFocusId: (returnFocusId) => set({ returnFocusId }),
  clearReturnFocusId: () => set({ returnFocusId: null }),
}));

export function isGameSessionActive(state: GameSessionState): boolean {
  return isInputFrozenState(state);
}
