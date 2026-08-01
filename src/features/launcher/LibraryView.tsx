import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import type { Game, GameStatus } from "../catalog/game-types";
import { useGames } from "../catalog/catalog-query";
import { useProductStore } from "../../stores/product-store";
import { useNavigationStore } from "../../stores/navigation-store";
import { useNavigation } from "../../ui/navigation/navigation-context";
import { NavigationGrid } from "../../ui/navigation/layouts/NavigationGrid";
import { ScrollRestoration } from "../../ui/navigation/scroll/scroll-restoration";
import { getWindowForTarget } from "../../ui/navigation/core/virtual-grid";
import { GameCard } from "./GameCard";
import { filterAndSortGames, type LibrarySort } from "./library-operations";

const LIBRARY_COLUMNS = 5;
const LIBRARY_VISIBLE_ROWS = 12;
const LIBRARY_OVERSCAN_ROWS = 2;
const LIBRARY_WINDOW_SIZE = LIBRARY_COLUMNS * LIBRARY_VISIBLE_ROWS;

interface VirtualScrollAnchor {
  focusId: string;
  top: number;
  scrollTop: number;
}

export function LibraryView({ onOpen }: { onOpen: (game: Game) => void }) {
  const { data: games = [] } = useGames();
  const { engine, registry } = useNavigation();
  const activeView = useProductStore((state) => state.activeView);
  const selectedGameId = useProductStore((state) => state.selectedGameId);
  const returnFocusId = useProductStore((state) => state.returnFocusId);
  const [query, setQuery] = useState("");
  const [status, setStatus] = useState<"all" | GameStatus>("all");
  const [sort, setSort] = useState<LibrarySort>("title");
  const [windowStart, setWindowStart] = useState(0);
  const windowStartRef = useRef(0);
  const pendingRestoreIndex = useRef<number | null>(null);
  const anchorRef = useRef<VirtualScrollAnchor | null>(null);
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
      const currentStart = windowStartRef.current;
      const nextWindow = getWindowForTarget(
        index,
        {
          start: currentStart,
          end: Math.min(
            filteredGames.length,
            currentStart + LIBRARY_WINDOW_SIZE,
          ),
        },
        {
          totalItems: filteredGames.length,
          columns: LIBRARY_COLUMNS,
          visibleRows: LIBRARY_VISIBLE_ROWS,
          overscanRows: LIBRARY_OVERSCAN_ROWS,
        },
      );
      if (nextWindow.start === currentStart) return;

      const activeFocusId = engine.getActiveFocusId();
      const anchorElement = activeFocusId
        ? registry.get(activeFocusId)?.element
        : undefined;
      const container = document.querySelector<HTMLElement>(
        '[data-scroll-scope="library"]',
      );
      if (anchorElement && container) {
        anchorRef.current = {
          focusId: activeFocusId ?? "",
          top: anchorElement.getBoundingClientRect().top,
          scrollTop: container.scrollTop,
        };
      }
      useNavigationStore.getState().updateDebug({
        windowStart: nextWindow.start,
        windowEnd: nextWindow.end,
        anchorFocusId: activeFocusId ?? undefined,
        scrollTopBefore: container?.scrollTop,
        scrollAuthority: "virtualization",
      });
      setWindowStart(nextWindow.start);
    },
    [engine, filteredGames.length, registry],
  );

  useEffect(() => {
    setWindowStart(0);
    pendingRestoreIndex.current = null;
    anchorRef.current = null;
  }, [query, sort, status]);

  useEffect(() => {
    const index = pendingRestoreIndex.current;
    if (index === null || !filteredGames[index]) return;
    const frame = window.requestAnimationFrame(() => {
      pendingRestoreIndex.current = null;
      engine.focus(`library-${filteredGames[index].id}`);
    });
    return () => window.cancelAnimationFrame(frame);
  }, [engine, filteredGames, windowStart]);

  useLayoutEffect(() => {
    const anchor = anchorRef.current;
    if (!anchor) return;
    const container = document.querySelector<HTMLElement>(
      '[data-scroll-scope="library"]',
    );
    const anchorElement = registry.get(anchor.focusId)?.element;
    if (container && anchorElement) {
      const nextTop = anchorElement.getBoundingClientRect().top;
      container.scrollTop += nextTop - anchor.top;
      useNavigationStore.getState().updateDebug({
        scrollTopAfter: container.scrollTop,
        scrollAuthority: "virtualization",
      });
    }
    registry.invalidateAll();
    anchorRef.current = null;
  }, [registry, windowStart]);

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
      const nextWindow = getWindowForTarget(
        index,
        {
          start: currentWindowStart,
          end: Math.min(
            filteredGames.length,
            currentWindowStart + LIBRARY_WINDOW_SIZE,
          ),
        },
        {
          totalItems: filteredGames.length,
          columns: LIBRARY_COLUMNS,
          visibleRows: LIBRARY_VISIBLE_ROWS,
          overscanRows: LIBRARY_OVERSCAN_ROWS,
        },
      );
      setWindowStart(nextWindow.start);
      pendingRestoreIndex.current = index;
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
          columns={LIBRARY_COLUMNS}
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
