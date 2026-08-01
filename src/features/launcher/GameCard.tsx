import type { Game } from "../catalog/game-types";
import { Focusable } from "../../ui/navigation/focus/Focusable";

interface GameCardProps {
  game: Game;
  focusId: string;
  onOpen: (game: Game) => void;
  compact?: boolean;
  gridIndex?: number;
}

export function GameCard({
  game,
  focusId,
  onOpen,
  compact = false,
  gridIndex,
}: GameCardProps) {
  return (
    <Focusable
      focusId={focusId}
      scopeId="product-shell"
      gridIndex={gridIndex}
      className={`game-card${compact ? " game-card-compact" : ""}`}
      ariaLabel={`${game.title}, ${game.platform}, ${game.status}`}
      onConfirm={() => onOpen(game)}
    >
      <img
        className="game-card-cover"
        src={game.coverUrl}
        alt=""
        draggable={false}
      />
      <span className="game-card-body">
        <strong>{game.title}</strong>
        <span className="game-card-meta">
          {game.platform} ·{" "}
          {game.status === "not-started" ? "New" : `${game.progress}%`}
        </span>
        {!compact && (
          <span className="game-card-status">
            {game.installed ? "Installed" : "Ready to add"}
          </span>
        )}
      </span>
    </Focusable>
  );
}
