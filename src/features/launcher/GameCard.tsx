import type { Game } from "../catalog/game-types";
import { Focusable } from "../../ui/navigation/focus/Focusable";
import { recordRender } from "../../ui/performance/performance-counters";

interface GameCardProps {
  game: Game;
  focusId: string;
  onOpen: (game: Game) => void;
  compact?: boolean;
  itemIndex?: number;
  gridIndex?: number;
}

export function GameCard({
  game,
  focusId,
  onOpen,
  compact = false,
  itemIndex,
  gridIndex,
}: GameCardProps) {
  recordRender("game-card");
  return (
    <Focusable
      focusId={focusId}
      scopeId="product-shell"
      itemIndex={itemIndex}
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
