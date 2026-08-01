import { useEffect } from "react";
import type { Game } from "../catalog/game-types";
import { useProductStore } from "../../stores/product-store";
import { useNavigation } from "../../ui/navigation/navigation-context";
import { NavigationRow } from "../../ui/navigation/layouts/NavigationRow";
import { GameCard } from "./GameCard";

interface HomeViewProps {
  games: Game[];
  onOpen: (game: Game) => void;
}

export function HomeView({ games, onOpen }: HomeViewProps) {
  const { engine } = useNavigation();
  const returnFocusId = useProductStore((state) => state.returnFocusId);
  const activeView = useProductStore((state) => state.activeView);
  const continuePlaying = games
    .filter((game) => game.status === "playing")
    .slice(0, 5);
  const recentlyPlayed = games.filter((game) => game.lastPlayedAt).slice(0, 5);
  const favorites = games.filter((game) => game.favorite).slice(0, 5);

  useEffect(() => {
    if (activeView !== "home") return;
    const focusId = returnFocusId?.startsWith("home-")
      ? returnFocusId
      : `home-continue-${continuePlaying[0]?.id ?? "empty"}`;
    if (document.querySelector(`[data-focus-id="${focusId}"]`)) {
      engine.focus(focusId);
    }
  }, [activeView, continuePlaying, engine, returnFocusId]);

  return (
    <section className="product-page home-view" aria-labelledby="home-heading">
      <div className="page-intro">
        <div>
          <p className="eyebrow">Your space</p>
          <h1 id="home-heading">Pick up where you left off.</h1>
        </div>
        <span className="page-hint">A calm library for quick sessions</span>
      </div>
      <GameRow
        title="Continue Playing"
        games={continuePlaying}
        prefix="home-continue"
        onOpen={onOpen}
      />
      <GameRow
        title="Recently Played"
        games={recentlyPlayed}
        prefix="home-recent"
        onOpen={onOpen}
      />
      <GameRow
        title="Favorites"
        games={favorites}
        prefix="home-favorite"
        onOpen={onOpen}
      />
    </section>
  );
}

function GameRow({
  title,
  games,
  prefix,
  onOpen,
}: {
  title: string;
  games: Game[];
  prefix: string;
  onOpen: (game: Game) => void;
}) {
  return (
    <section className="game-row" aria-labelledby={`${prefix}-heading`}>
      <div className="row-heading">
        <h2 id={`${prefix}-heading`}>{title}</h2>
        <span>{games.length} shown</span>
      </div>
      {games.length ? (
        <NavigationRow groupId={prefix}>
          {games.map((game) => (
            <GameCard
              key={game.id}
              game={game}
              focusId={`${prefix}-${game.id}`}
              onOpen={onOpen}
              compact
            />
          ))}
        </NavigationRow>
      ) : (
        <p className="empty-state">No games in this row yet.</p>
      )}
    </section>
  );
}
