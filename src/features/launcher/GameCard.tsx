import type { Game } from "../catalog/game-types";
import { Focusable } from "../../ui/navigation/focus/Focusable";
import { recordRender } from "../../ui/performance/performance-counters";
import { MediaImage } from "../../ui/performance/MediaImage";

interface GameCardProps {
  game: Game;
  focusId: string;
  onOpen: (game: Game) => void;
  compact?: boolean;
  vertical?: boolean;
  itemIndex?: number;
  gridIndex?: number;
}

export function GameCard({
  game,
  focusId,
  onOpen,
  compact = false,
  vertical = false,
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
      <MediaImage
        gameId={game.id}
        mediaType="grid"
        className="game-card-cover"
        src={
          compact && !vertical
            ? game.coverUrl
            : game.verticalCoverUrl || game.coverUrl
        }
        alt=""
        draggable={false}
      />
      {compact && (
        <span className="game-card-overlay" aria-hidden="true">
          <span className="game-card-badge">PC</span>
          <span className="game-card-playtime">
            <span>{"\u25f7"}</span> {formatCardPlaytime(game.playtimeMinutes)}
          </span>
        </span>
      )}
      <span className="game-card-body">
        <strong>{game.title}</strong>
        <span className="game-card-meta">
          {game.platform} {"\u00b7"}{" "}
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

function formatCardPlaytime(minutes: number): string {
  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;
  return hours > 0 ? `${hours}h ${remainingMinutes}m` : `${minutes}m`;
}
