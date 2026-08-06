import { invoke } from "@tauri-apps/api/core";
import type { ActivityFriend, ActivitySnapshot } from "./activity-types";

export const activityService = {
  get(gameId: string): Promise<ActivitySnapshot> {
    if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) {
      return Promise.reject(new Error("ACTIVITY_RUNTIME_UNAVAILABLE"));
    }
    return invoke<ActivitySnapshot>("get_game_activity", { gameId });
  },

  getFriends(gameId: string): Promise<ActivityFriend[]> {
    return invoke<ActivityFriend[]>("get_game_activity_friends", { gameId });
  },

  startSession(gameId: string): Promise<number> {
    return invoke<number>("start_game_session", { gameId });
  },

  endSession(
    gameId: string,
    sessionId: number,
    interrupted = false,
  ): Promise<void> {
    return invoke<void>("end_game_session", {
      gameId,
      sessionId,
      interrupted,
    });
  },
};

export function activityErrorMessage(error: unknown): string {
  const code = error instanceof Error ? error.message : String(error);
  switch (code) {
    case "ACTIVITY_RUNTIME_UNAVAILABLE":
      return "La actividad requiere la aplicación de escritorio.";
    case "ACCOUNT_NOT_CONFIGURED":
      return "Steam no está configurado; la actividad local sigue disponible.";
    case "STEAM_OFFLINE":
      return "Steam no está disponible; se muestran los datos locales.";
    case "STEAM_INVALID_RESPONSE":
    case "STEAM_API_ERROR":
      return "Steam no pudo devolver la actividad social.";
    case "GAME_NOT_FOUND":
      return "No se encontró el juego en la biblioteca local.";
    default:
      return "No se pudo consultar la actividad.";
  }
}
