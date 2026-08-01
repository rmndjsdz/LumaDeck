import { useEffect, useMemo } from "react";
import { useGames } from "../catalog/catalog-query";
import type { Game } from "../catalog/game-types";
import { useProductStore, type ProductView } from "../../stores/product-store";
import { useNavigation } from "../../ui/navigation/navigation-context";
import { FocusScope } from "../../ui/navigation/focus/FocusScope";
import { Focusable } from "../../ui/navigation/focus/Focusable";
import { NavigationDebugOverlay } from "../../ui/navigation/debug/NavigationDebugOverlay";
import { PerformanceOverlay } from "../../ui/performance/PerformanceOverlay";
import { BackgroundView } from "../../ui/background/BackgroundView";
import { DetailsView } from "./DetailsView";
import { HomeView } from "./HomeView";
import { LibraryView } from "./LibraryView";

export function ProductShell() {
  const { engine } = useNavigation();
  const { data: games = [], isPending, isError } = useGames();
  const activeView = useProductStore((state) => state.activeView);
  const selectedGameId = useProductStore((state) => state.selectedGameId);
  const returnView = useProductStore((state) => state.returnView);
  const setView = useProductStore((state) => state.setView);
  const openDetails = useProductStore((state) => state.openDetails);
  const selectedGame = games.find((game) => game.id === selectedGameId);
  const backgroundGame =
    selectedGame ?? games.find((game) => game.status === "playing") ?? games[0];
  const backgroundUrls = useMemo(() => {
    if (!backgroundGame) return [];
    const index = games.findIndex((game) => game.id === backgroundGame.id);
    return [games[index - 1]?.backgroundUrl, games[index + 1]?.backgroundUrl];
  }, [backgroundGame, games]);

  useEffect(() => {
    if (activeView === "details" && selectedGameId && !selectedGame)
      setView(returnView);
  }, [activeView, returnView, selectedGame, selectedGameId, setView]);

  const navigate = (view: ProductView) => {
    if (view !== "library") engine.cancelPendingVirtualFocus("view-change");
    setView(view);
  };
  const handleOpen = (game: Game) => {
    const openerFocusId = engine.getActiveFocusId();
    engine.prepareScopeOpen("details", openerFocusId ?? undefined);
    openDetails(game.id, activeView, openerFocusId);
  };

  return (
    <div className="app-shell">
      <BackgroundView
        url={backgroundGame?.backgroundUrl ?? null}
        preloadUrls={backgroundUrls}
      />
      <FocusScope
        scopeId="product-shell"
        initialFocusId="shell-home"
        restoreFocus
        rememberScroll
        activateOnMount
      >
        <header className="app-header">
          <div className="brand-lockup">
            <span className="brand-mark">L</span>
            <span>LumaDeck</span>
          </div>
          <nav className="primary-nav" aria-label="Primary navigation">
            <Focusable
              focusId="shell-home"
              scopeId="product-shell"
              className="shell-nav-button"
              ariaCurrent={activeView === "home" ? "page" : false}
              onConfirm={() => navigate("home")}
            >
              Home
            </Focusable>
            <Focusable
              focusId="shell-library"
              scopeId="product-shell"
              className="shell-nav-button"
              ariaCurrent={activeView === "library" ? "page" : false}
              onConfirm={() => navigate("library")}
            >
              Library
            </Focusable>
          </nav>
          <span className="shell-status">LOCAL CATALOG · 200</span>
        </header>
        <main className="app-shell-main">
          {isPending && (
            <p className="loading-state">Preparing your library…</p>
          )}
          {isError && (
            <p className="empty-state">Could not load the local catalog.</p>
          )}
          {!isPending && !isError && activeView === "home" && (
            <HomeView games={games} onOpen={handleOpen} />
          )}
          {!isPending && !isError && activeView === "library" && (
            <LibraryView onOpen={handleOpen} />
          )}
          {!isPending && !isError && activeView === "details" && (
            <DetailsView game={selectedGame} />
          )}
        </main>
        <footer className="app-footer">
          <span>↑ ↓ ← → navigate</span>
          <span>Enter / Space select</span>
          <span>Esc back</span>
          <span className="footer-spacer" />
          <span>Local-first launcher slice</span>
        </footer>
      </FocusScope>
      <NavigationDebugOverlay />
      <PerformanceOverlay />
    </div>
  );
}
