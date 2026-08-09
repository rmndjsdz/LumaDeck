import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { GameSessionStatus } from "./game-session-types";

const EVENT_NAME = "game-session-state";

function ensureDesktopRuntime(): void {
  if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) {
    throw new Error("GAME_SESSION_RUNTIME_UNAVAILABLE");
  }
}

function isDesktopRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function idleStatus(): GameSessionStatus {
  return {
    sessionId: "",
    gameId: "",
    steamAppId: 0,
    source: "none",
    state: "idle",
    occurredAt: new Date().toISOString(),
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
  };
}

export const gameSessionService = {
  start(gameId: string): Promise<GameSessionStatus> {
    ensureDesktopRuntime();
    return invoke<GameSessionStatus>("start_game_play", {
      gameId,
    });
  },

  current(): Promise<GameSessionStatus> {
    if (!isDesktopRuntime()) {
      return Promise.resolve(idleStatus());
    }
    ensureDesktopRuntime();
    return invoke<GameSessionStatus>("get_game_session");
  },

  dismiss(): Promise<GameSessionStatus> {
    ensureDesktopRuntime();
    return invoke<GameSessionStatus>("dismiss_game_session");
  },

  minimize(): Promise<void> {
    ensureDesktopRuntime();
    return invoke<void>("minimize_lumadeck_window");
  },

  restore(): Promise<void> {
    ensureDesktopRuntime();
    return invoke<void>("restore_lumadeck_window");
  },

  subscribe(
    listener: (status: GameSessionStatus) => void,
  ): Promise<() => void> {
    if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) {
      return Promise.resolve(() => undefined);
    }
    return listen<GameSessionStatus>(EVENT_NAME, (event) =>
      listener(event.payload),
    );
  },
};

export function gameSessionErrorMessage(error: unknown): string {
  const code = error instanceof Error ? error.message : String(error);
  if (code === "ANOTHER_GAME_SESSION_ACTIVE") {
    return "Ya existe otra sesión de juego activa.";
  }
  if (code === "GAME_SESSION_RUNTIME_UNAVAILABLE") {
    return "El lanzamiento requiere la aplicación de escritorio.";
  }
  return "No se pudo iniciar la sesión del juego.";
}
