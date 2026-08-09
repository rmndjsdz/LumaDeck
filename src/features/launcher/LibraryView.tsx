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
import {
  filterAndSortGames,
  LIBRARY_MORE_GENRE_FILTERS,
  LIBRARY_PRIMARY_GENRE_FILTERS,
} from "./library-operations";
import { navigationRuntimeTrace } from "../../ui/navigation/debug/navigation-runtime-trace";

const DEFAULT_LIBRARY_COLUMNS = 6;
const MIN_LIBRARY_COLUMNS = 2;
const MAX_LIBRARY_COLUMNS = 7;
const LIBRARY_MIN_CARD_WIDTH = 240;
const LIBRARY_GRID_GAP = 14;
const LIBRARY_VISIBLE_ROWS = 12;
const LIBRARY_OVERSCAN_ROWS = 2;

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
  const activeFocusId = useNavigationStore((state) => state.activeFocusId);
  const selectedGameId = useProductStore((state) => state.selectedGameId);
  const returnFocusId = useProductStore((state) => state.returnFocusId);
  const query = useLibraryStore((state) => state.query);
  const status = useLibraryStore((state) => state.status);
  const sort = useLibraryStore((state) => state.sort);
  const genre = useLibraryStore((state) => state.genre);
  const queryVersion = useLibraryStore((state) => state.queryVersion);
  const queryCommitted = useLibraryStore((state) => state.queryCommitted);
  const setQuery = useLibraryStore((state) => state.setQuery);
  const setStatus = useLibraryStore((state) => state.setStatus);
  const setSort = useLibraryStore((state) => state.setSort);
  const setGenre = useLibraryStore((state) => state.setGenre);
  const [windowStart, setWindowStart] = useState(0);
  const [gridColumns, setGridColumns] = useState(DEFAULT_LIBRARY_COLUMNS);
  const [showMoreGenres, setShowMoreGenres] = useState(false);
  const gridViewportRef = useRef<HTMLDivElement>(null);
  const resultGenerationRef = useRef(0);
  const contentSignatureRef = useRef<string | null>(null);
  const windowStartRef = useRef(0);
  const anchorRef = useRef<VirtualScrollAnchor | null>(null);
  windowStartRef.current = windowStart;
  const libraryWindowSize = gridColumns * LIBRARY_VISIBLE_ROWS;
  const filteredGames = useMemo(
    () => filterAndSortGames(games, query, status, sort, genre),
    [games, genre, query, sort, status],
  );
  const visibleGenreFilters = useMemo(() => {
    if (showMoreGenres) {
      return [...LIBRARY_PRIMARY_GENRE_FILTERS, ...LIBRARY_MORE_GENRE_FILTERS];
    }
    const selectedMoreGenre = LIBRARY_MORE_GENRE_FILTERS.find(
      (filter) => filter.id === genre,
    );
    return selectedMoreGenre
      ? [...LIBRARY_PRIMARY_GENRE_FILTERS, selectedMoreGenre]
      : LIBRARY_PRIMARY_GENRE_FILTERS;
  }, [genre, showMoreGenres]);
  const visibleGames = filteredGames.slice(
    windowStart,
    windowStart + libraryWindowSize,
  );
  useLayoutEffect(() => {
    const element = gridViewportRef.current;
    if (!element) return;

    const updateColumns = () => {
      const width = element.clientWidth;
      if (width <= 0) return;
      const nextColumns = Math.max(
        MIN_LIBRARY_COLUMNS,
        Math.min(
          MAX_LIBRARY_COLUMNS,
          Math.floor(
            (width + LIBRARY_GRID_GAP) /
              (LIBRARY_MIN_CARD_WIDTH + LIBRARY_GRID_GAP),
          ),
        ),
      );
      setGridColumns((current) =>
        current === nextColumns ? current : nextColumns,
      );
    };

    updateColumns();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(updateColumns);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

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
    const contentSignature = `${queryVersion}:${query}:${status}:${genre}:${sort}:${games.length}`;
    if (contentSignatureRef.current !== contentSignature) {
      contentSignatureRef.current = contentSignature;
      resultGenerationRef.current += 1;
    }
    navigationRuntimeTrace.recordLibraryContent({
      queryVersion,
      queryLength: query.length,
      queryCommitted,
      filterIds: [status, genre],
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
    genre,
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
          end: Math.min(filteredGames.length, currentStart + libraryWindowSize),
        },
        {
          totalItems: filteredGames.length,
          columns: gridColumns,
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
    [engine, filteredGames.length, gridColumns, libraryWindowSize, registry],
  );

  useEffect(() => {
    setWindowStart(0);
    anchorRef.current = null;
  }, [genre, gridColumns, query, sort, status]);

  useLayoutEffect(() => {
    if (activeView !== "library" || !activeFocusId?.startsWith("library-")) {
      return;
    }
    if (!activeFocusId.startsWith("library-game-")) return;
    const stillVisible = filteredGames.some(
      (game) => `library-${game.id}` === activeFocusId,
    );
    if (stillVisible) return;

    const fallbackFocusId = filteredGames[0]
      ? `library-${filteredGames[0].id}`
      : `library-genre-${genre}`;
    if (engine.getActiveFocusId() !== fallbackFocusId) {
      engine.focus(fallbackFocusId);
    }
  }, [activeFocusId, activeView, engine, filteredGames, genre]);

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
      index >= currentWindowStart + libraryWindowSize
    ) {
      const nextWindow = getWindowForTarget(
        index,
        {
          start: currentWindowStart,
          end: Math.min(
            filteredGames.length,
            currentWindowStart + libraryWindowSize,
          ),
        },
        {
          totalItems: filteredGames.length,
          columns: gridColumns,
          visibleRows: LIBRARY_VISIBLE_ROWS,
          overscanRows: LIBRARY_OVERSCAN_ROWS,
        },
      );
      setWindowStart(nextWindow.start);
      return;
    }
  }, [
    activeView,
    engine,
    filteredGames,
    gridColumns,
    libraryWindowSize,
    returnFocusId,
    selectedGameId,
  ]);

  return (
    <section
      className="product-page library-view"
      aria-labelledby="library-heading"
    >
      <header className="library-header">
        <div className="page-intro">
          <div>
            <p className="eyebrow">{filteredGames.length} games</p>
            <h1 id="library-heading">Library</h1>
            <p className="library-header-subtitle">
              Encuentra y organiza todos tus juegos en un solo lugar.
            </p>
          </div>
        </div>
      </header>
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
            childRegionId: "library-genres",
            entryFocusId: "library-genre-all",
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
          <div className="library-filter-statuses">
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
          </div>
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
        <div className="library-genre-row">
          <span className="library-genre-label">Géneros</span>
          <NavigationTabs
            groupId="library-genres"
            ariaLabel="Library genres"
            selectedId={`library-genre-${genre}`}
            onSelect={(focusId) => {
              const selectedGenre = visibleGenreFilters.find(
                (filter) => `library-genre-${filter.id}` === focusId,
              );
              if (selectedGenre) setGenre(selectedGenre.id);
            }}
            upTargetId="library-filter-all"
            navigationRegion={{
              regionId: "library-genres",
              parentRegionId: "library-filters",
              childRegionId: "library-content",
              entryFocusId: filteredGames[0]
                ? `library-${filteredGames[0].id}`
                : `library-genre-${genre}`,
            }}
            className="library-genre-nav"
          >
            {visibleGenreFilters.map((filter) => (
              <NavigationTab
                key={filter.id}
                focusId={`library-genre-${filter.id}`}
                scopeId="product-shell"
                className="library-genre-chip"
              >
                <span
                  className={`library-genre-icon library-genre-icon-${filter.id}`}
                  aria-hidden="true"
                >
                  {filter.icon}
                </span>
                {filter.label}
              </NavigationTab>
            ))}
            <Focusable
              focusId="library-genre-more"
              scopeId="product-shell"
              className="library-genre-chip library-genre-more"
              ariaLabel={
                showMoreGenres ? "Mostrar menos géneros" : "Más géneros"
              }
              onConfirm={() => setShowMoreGenres((current) => !current)}
            >
              <span className="library-genre-icon" aria-hidden="true">
                {showMoreGenres ? "−" : "+"}
              </span>
              {showMoreGenres ? "Menos" : "Más"}
            </Focusable>
          </NavigationTabs>
        </div>
      </div>
      <ScrollRestoration scopeId="library" className="library-scroll-area">
        <div ref={gridViewportRef} className="library-grid-viewport">
          <NavigationGrid
            groupId="library-grid"
            columns={gridColumns}
            itemCount={filteredGames.length}
            onRequestIndex={requestIndex}
            resolveFocusId={(index) =>
              `library-${filteredGames[index]?.id ?? ""}`
            }
            regionId="library-content"
            parentRegionId="library-genres"
            entryFocusId={
              filteredGames[0]
                ? `library-${filteredGames[0].id}`
                : `library-genre-${genre}`
            }
            exitFocusId={`library-genre-${genre}`}
            gamepadParentRegionId="library-genres"
            gamepadExitFocusId={`library-genre-${genre}`}
          >
            {visibleGames.map((game, index) => (
              <GameCard
                key={game.id}
                game={game}
                focusId={`library-${game.id}`}
                onOpen={onOpen}
                gridIndex={windowStart + index}
                compact
              />
            ))}
          </NavigationGrid>
        </div>
        {!filteredGames.length && (
          <p className="library-empty-state">
            No hay juegos que coincidan con estos filtros.
          </p>
        )}
      </ScrollRestoration>
    </section>
  );
}
