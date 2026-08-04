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
import { useLibraryStore } from "../../stores/library-store";
import { useProductStore } from "../../stores/product-store";
import { useNavigationStore } from "../../stores/navigation-store";
import { useNavigation } from "../../ui/navigation/navigation-context";
import { NavigationGrid } from "../../ui/navigation/layouts/NavigationGrid";
import {
  NavigationTab,
  NavigationTabs,
} from "../../ui/navigation/layouts/NavigationTabs";
import { ScrollRestoration } from "../../ui/navigation/scroll/scroll-restoration";
import { Focusable } from "../../ui/navigation/focus/Focusable";
import { GamepadTextInput } from "../../ui/keyboard/GamepadTextInput";
import { getWindowForTarget } from "../../ui/navigation/core/virtual-grid";
import { GameCard } from "./GameCard";
import { filterAndSortGames } from "./library-operations";
import { navigationRuntimeTrace } from "../../ui/navigation/debug/navigation-runtime-trace";

const LIBRARY_COLUMNS = 5;
const LIBRARY_VISIBLE_ROWS = 12;
const LIBRARY_OVERSCAN_ROWS = 2;
const LIBRARY_WINDOW_SIZE = LIBRARY_COLUMNS * LIBRARY_VISIBLE_ROWS;

const STATUS_FILTERS: readonly {
  id: string;
  label: string;
  value: "all" | GameStatus;
}[] = [
  { id: "all", label: "Todos", value: "all" },
  { id: "playing", label: "Instalados", value: "playing" },
  { id: "not-started", label: "Sin empezar", value: "not-started" },
  { id: "completed", label: "Completados", value: "completed" },
];

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
  const query = useLibraryStore((state) => state.query);
  const status = useLibraryStore((state) => state.status);
  const sort = useLibraryStore((state) => state.sort);
  const queryVersion = useLibraryStore((state) => state.queryVersion);
  const queryCommitted = useLibraryStore((state) => state.queryCommitted);
  const setQuery = useLibraryStore((state) => state.setQuery);
  const setStatus = useLibraryStore((state) => state.setStatus);
  const setSort = useLibraryStore((state) => state.setSort);
  const [windowStart, setWindowStart] = useState(0);
  const resultGenerationRef = useRef(0);
  const contentSignatureRef = useRef<string | null>(null);
  const windowStartRef = useRef(0);
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

  const handleQueryChange = useCallback(
    (value: string) => {
      setQuery(value);
    },
    [setQuery],
  );

  useEffect(() => {
    if (activeView !== "library") return;
    navigationRuntimeTrace.recordLibraryLifecycle("mounted");
    return () => navigationRuntimeTrace.recordLibraryLifecycle("unmounted");
  }, [activeView]);

  useEffect(() => {
    if (activeView !== "library") return;
    const contentSignature = `${queryVersion}:${query}:${status}:${sort}:${games.length}`;
    if (contentSignatureRef.current !== contentSignature) {
      contentSignatureRef.current = contentSignature;
      resultGenerationRef.current += 1;
    }
    navigationRuntimeTrace.recordLibraryContent({
      queryVersion,
      queryLength: query.length,
      queryCommitted,
      filterIds: [status],
      sortId: sort,
      resultCount: filteredGames.length,
      visibleResultIds: visibleGames.map((game) => game.id),
      resultGeneration: resultGenerationRef.current,
      resultIds: filteredGames.map((game) => game.id),
    });
  }, [
    activeView,
    filteredGames,
    games.length,
    query,
    queryCommitted,
    queryVersion,
    sort,
    status,
    visibleGames,
  ]);

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
    anchorRef.current = null;
  }, [query, sort, status]);

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
    const selectedId =
      engine.getLastFocusedFocusId("library-content") ??
      (returnFocusId?.startsWith("library-")
        ? returnFocusId
        : `library-${selectedGameId ?? filteredGames[0].id}`);
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
      return;
    }
  }, [activeView, engine, filteredGames, returnFocusId, selectedGameId]);

  return (
    <section
      className="product-page library-view"
      aria-labelledby="library-heading"
    >
      <div className="page-intro">
        <div>
          <p className="eyebrow">{games.length} local entries</p>
          <h1 id="library-heading">Library</h1>
        </div>
        <span className="page-hint">{filteredGames.length} games match</span>
      </div>
      <div className="library-toolbar" aria-label="Library filters">
        <NavigationTabs
          groupId="library-filters"
          ariaLabel="Library filters"
          selectedId={`library-filter-${status}`}
          onSelect={(focusId) => {
            const filter = STATUS_FILTERS.find(
              (candidate) => `library-filter-${candidate.id}` === focusId,
            );
            if (filter) setStatus(filter.value);
          }}
          navigationRegion={{
            regionId: "library-filters",
            parentRegionId: "main-navigation",
            childRegionId: "library-content",
            entryFocusId: "library-game-001",
          }}
          className="library-filter-nav"
        >
          <div className="library-search-control">
            <span>Buscar</span>
            <GamepadTextInput
              focusId="library-search"
              scopeId="product-shell"
              value={query}
              onChange={handleQueryChange}
              placeholder="Search games"
              ariaLabel="Buscar juegos"
              className="library-search-input"
            />
          </div>
          {STATUS_FILTERS.map((filter) => (
            <NavigationTab
              key={filter.id}
              focusId={`library-filter-${filter.id}`}
              scopeId="product-shell"
              className="library-filter-button"
            >
              {filter.label}
            </NavigationTab>
          ))}
          <Focusable
            focusId="library-sort"
            scopeId="product-shell"
            className="library-filter-button library-sort-button"
            ariaLabel={`Ordenar: ${sort}`}
            onConfirm={() =>
              setSort(
                sort === "title"
                  ? "recent"
                  : sort === "recent"
                    ? "time"
                    : "title",
              )
            }
          >
            Orden:{" "}
            {sort === "title"
              ? "Título"
              : sort === "recent"
                ? "Reciente"
                : "Tiempo"}
          </Focusable>
          <Focusable
            focusId="library-clear-filters"
            scopeId="product-shell"
            className="library-filter-button library-clear-button"
            ariaLabel="Limpiar filtros"
            onConfirm={() => useLibraryStore.getState().reset()}
          >
            Limpiar
          </Focusable>
        </NavigationTabs>
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
          regionId="library-content"
          parentRegionId="main-navigation"
          entryFocusId="library-game-001"
          exitFocusId="main-nav-library"
          gamepadParentRegionId="library-filters"
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
