import { useEffect, useLayoutEffect, useState } from "react";
import type { Game } from "../catalog/game-types";
import { Focusable } from "../../ui/navigation/focus/Focusable";
import { useNavigation } from "../../ui/navigation/navigation-context";
import { useGameSessionStore } from "./game-session-store";
import { gameSessionService } from "./game-session-service";

export function GameSessionScreen({ games }: { games: Game[] }) {
  const { engine, inputManager } = useNavigation();
  const currentState = useGameSessionStore((state) => state.currentState);
  const gameId = useGameSessionStore((state) => state.gameId);
  const returnFocusId = useGameSessionStore((state) => state.returnFocusId);
  const inputFrozen = useGameSessionStore((state) => state.inputFrozen);
  const message = useGameSessionStore((state) => state.message);
  const unsupportedReason = useGameSessionStore(
    (state) => state.unsupportedReason,
  );
  const elapsedSeconds = useGameSessionStore((state) => state.elapsedSeconds);
  const applyStatus = useGameSessionStore((state) => state.applyStatus);
  const setGamePresentation = useGameSessionStore(
    (state) => state.setGamePresentation,
  );
  const clearReturnFocusId = useGameSessionStore(
    (state) => state.clearReturnFocusId,
  );
  const [dismissError, setDismissError] = useState<string | null>(null);
  const game = games.find((candidate) => candidate.id === gameId);
  const dismissScopeId = returnFocusId?.startsWith("details-")
    ? "details"
    : "product-shell";

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void gameSessionService
      .subscribe((status) => {
        if (!disposed) applyStatus(status);
      })
      .then((stop) => {
        if (disposed) {
          stop();
        } else {
          unlisten = stop;
        }
      });
    void gameSessionService
      .current()
      .then((status) => {
        if (!disposed) applyStatus(status);
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [applyStatus]);

  useEffect(() => {
    if (game) setGamePresentation(game.title, game.backgroundUrl);
  }, [game, setGamePresentation]);

  useEffect(() => {
    inputManager.setInputFrozen(inputFrozen);
  }, [inputFrozen, inputManager]);

  useEffect(() => () => inputManager.setInputFrozen(false), [inputManager]);

  useEffect(() => {
    if (currentState === "running") {
      const timer = window.setTimeout(() => {
        void gameSessionService.minimize().catch(() => undefined);
      }, 800);
      return () => window.clearTimeout(timer);
    }
    if (
      currentState === "finishing" ||
      currentState === "error" ||
      currentState === "unsupported"
    ) {
      void gameSessionService.restore().catch(() => undefined);
    }
  }, [currentState]);

  useLayoutEffect(() => {
    if (currentState === "error" || currentState === "unsupported") {
      engine.focus("game-session-dismiss");
      return;
    }
    if (currentState === "idle" && returnFocusId) {
      engine.focus(returnFocusId);
      clearReturnFocusId();
    }
  }, [clearReturnFocusId, currentState, engine, returnFocusId]);

  if (currentState === "idle") return null;

  const title = game?.title ?? "Juego";
  const detail =
    currentState === "preparing"
      ? message || "Comprobando instalación y compatibilidad…"
      : currentState === "launching"
        ? "Steam está preparando el juego…"
        : currentState === "running"
          ? `Tiempo registrado: ${formatElapsed(elapsedSeconds)}`
          : currentState === "finishing"
            ? "Esperando el cierre completo del juego."
            : (unsupportedReason ?? message);

  return (
    <div
      className="game-session-overlay"
      role="status"
      aria-live="polite"
      data-session-state={currentState}
    >
      <div className="game-session-panel">
        {game?.backgroundUrl && (
          <div
            className="game-session-art"
            aria-hidden="true"
            style={{ backgroundImage: `url("${game.backgroundUrl}")` }}
          />
        )}
        <div className="game-session-content">
          <p className="eyebrow">LumaDeck</p>
          <h1>
            {currentState === "unsupported"
              ? "Juego no compatible todavía"
              : currentState === "error"
                ? "No se pudo iniciar"
                : currentState === "finishing"
                  ? "Finalizando sesión…"
                  : currentState === "running"
                    ? "Juego iniciado"
                    : currentState === "launching"
                      ? "Iniciando"
                      : "Preparando el juego"}
          </h1>
          <p className="game-session-title">{title}</p>
          <p className="game-session-message">{detail}</p>
          {(currentState === "error" || currentState === "unsupported") && (
            <Focusable
              focusId="game-session-dismiss"
              scopeId={dismissScopeId}
              className="primary-button game-session-dismiss"
              onConfirm={() => {
                setDismissError(null);
                void gameSessionService.dismiss().catch((error: unknown) => {
                  setDismissError(String(error));
                });
              }}
            >
              Volver
            </Focusable>
          )}
          {dismissError && (
            <p className="game-session-error" role="alert">
              {dismissError}
            </p>
          )}
        </div>
      </div>
    </div>
  );
}

function formatElapsed(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;
  return `${minutes}m ${remainingSeconds}s`;
}
