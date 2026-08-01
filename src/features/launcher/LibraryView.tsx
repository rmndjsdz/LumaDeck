import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { Game, GameStatus } from "../catalog/game-types";
import { useGames } from "../catalog/catalog-query";
import { useProductStore } from "../../stores/product-store";
import { useNavigation } from "../../ui/navigation/navigation-context";
import { NavigationGrid } from "../../ui/navigation/layouts/NavigationGrid";
import { ScrollRestoration } from "../../ui/navigation/scroll/scroll-restoration";
import { GameCard } from "./GameCard";
import { filterAndSortGames, type LibrarySort } from "./library-operations";

const LIBRARY_WINDOW_SIZE = 60;

export function LibraryView({ onOpen }: { onOpen: (game: Game) => void }) {
  const { data: games = [] } = useGames();
  const { engine } = useNavigation();
  const activeView = useProductStore((state) => state.activeView);
  const selectedGameId = useProductStore((state) => state.selectedGameId);
  const returnFocusId = useProductStore((state) => state.returnFocusId);
  const [query, setQuery] = useState("");
  const [status, setStatus] = useState<"all" | GameStatus>("all");
  const [sort, setSort] = useState<LibrarySort>("title");
  const [windowStart, setWindowStart] = useState(0);
  const windowStartRef = useRef(0);
  const pendingIndex = useRef<number | null>(null);
  windowStartRef.current = windowStart;
  const filteredGames = useMemo(
    () => filterAndSortGames(games, query, status, sort),
    [games, query, sort, status],
  );
  const visibleGames = filteredGames.slice(
    windowStart,
    windowStart + LIBRARY_WINDOW_SIZE,
  );

  const requestIndex = useCallback(
    (index: number) => {
      pendingIndex.current = index;
      setWindowStart(
        Math.max(
          0,
          Math.min(
            index - Math.floor(LIBRARY_WINDOW_SIZE / 2),
            Math.max(0, filteredGames.length - LIBRARY_WINDOW_SIZE),
          ),
        ),
      );
    },
    [filteredGames.length],
  );

  useEffect(() => {
    setWindowStart(0);
    pendingIndex.current = null;
  }, [query, sort, status]);

  useEffect(() => {
    const index = pendingIndex.current;
    if (index === null || !filteredGames[index]) return;
    pendingIndex.current = null;
    const frame = window.requestAnimationFrame(() => {
      engine.focus(`library-${filteredGames[index].id}`);
    });
    return () => window.cancelAnimationFrame(frame);
  }, [engine, filteredGames, windowStart]);

  useEffect(() => {
    if (activeView !== "library" || !filteredGames.length) return;
    const selectedId = returnFocusId?.startsWith("library-")
      ? returnFocusId
      : `library-${selectedGameId ?? filteredGames[0].id}`;
    const selectedIndex = filteredGames.findIndex(
      (game) => `library-${game.id}` === selectedId,
    );
    const index = selectedIndex >= 0 ? selectedIndex : 0;
    const currentWindowStart = windowStartRef.current;
    if (
      index < currentWindowStart ||
      index >= currentWindowStart + LIBRARY_WINDOW_SIZE
    ) {
      setWindowStart(
        Math.max(
          0,
          Math.min(
            index - Math.floor(LIBRARY_WINDOW_SIZE / 2),
            Math.max(0, filteredGames.length - LIBRARY_WINDOW_SIZE),
          ),
        ),
      );
      pendingIndex.current = index;
      return;
    }
    const frame = window.requestAnimationFrame(() => {
      if (document.querySelector(`[data-focus-id="${selectedId}"]`)) {
        engine.focus(selectedId);
      }
    });
    return () => window.cancelAnimationFrame(frame);
  }, [activeView, engine, filteredGames, returnFocusId, selectedGameId]);

  return (
    <section
      className="product-page library-view"
      aria-labelledby="library-heading"
    >
      <div className="page-intro">
        <div>
          <p className="eyebrow">200 local entries</p>
          <h1 id="library-heading">Library</h1>
        </div>
        <span className="page-hint">{filteredGames.length} games match</span>
      </div>
      <div className="library-toolbar" aria-label="Library filters">
        <label>
          <span>Filter title</span>
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search games"
          />
        </label>
        <label>
          <span>Status</span>
          <select
            value={status}
            onChange={(event) =>
              setStatus(event.target.value as "all" | GameStatus)
            }
          >
            <option value="all">All statuses</option>
            <option value="playing">Playing</option>
            <option value="not-started">Not started</option>
            <option value="completed">Completed</option>
          </select>
        </label>
        <label>
          <span>Sort</span>
          <select
            value={sort}
            onChange={(event) => setSort(event.target.value as LibrarySort)}
          >
            <option value="title">Title</option>
            <option value="recent">Recent</option>
            <option value="time">Time played</option>
          </select>
        </label>
      </div>
      <ScrollRestoration scopeId="library" className="library-scroll-area">
        <NavigationGrid
          groupId="library-grid"
          columns={5}
          itemCount={filteredGames.length}
          onRequestIndex={requestIndex}
          resolveFocusId={(index) =>
            `library-${filteredGames[index]?.id ?? ""}`
          }
        >
          {visibleGames.map((game, index) => (
            <GameCard
              key={game.id}
              game={game}
              focusId={`library-${game.id}`}
              onOpen={onOpen}
              gridIndex={windowStart + index}
            />
          ))}
        </NavigationGrid>
      </ScrollRestoration>
    </section>
  );
}
