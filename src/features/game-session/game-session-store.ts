import { create } from "zustand";
import type { GameSessionState, GameSessionStatus } from "./game-session-types";

interface GameSessionStore {
  currentState: GameSessionState;
  sessionId: string;
  gameId: string;
  steamAppId: number;
  title: string;
  artwork: string;
  elapsedSeconds: number;
  message: string;
  unsupportedReason: string | null;
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
  title: "",
  artwork: "",
  elapsedSeconds: 0,
  message: "",
  unsupportedReason: null,
  inputFrozen: false,
  returnFocusId: null,
  applyStatus: (status) =>
    set({
      currentState: status.state,
      sessionId: status.sessionId,
      gameId: status.gameId,
      steamAppId: status.steamAppId,
      elapsedSeconds: status.elapsedSeconds,
      message: status.message,
      unsupportedReason: status.unsupportedReason ?? null,
      inputFrozen: isInputFrozenState(status.state),
    }),
  setGamePresentation: (title, artwork) => set({ title, artwork }),
  setReturnFocusId: (returnFocusId) => set({ returnFocusId }),
  clearReturnFocusId: () => set({ returnFocusId: null }),
}));

export function isGameSessionActive(state: GameSessionState): boolean {
  return isInputFrozenState(state);
}
