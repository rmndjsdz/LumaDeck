import { useEffect } from "react";
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
import { ViewTransition } from "../../ui/motion/ViewTransition";
import { recordRender } from "../../ui/performance/performance-counters";
import { markPerformance } from "../../ui/performance/performance-marks";

export function ProductShell() {
  recordRender("app-shell");
  const { engine } = useNavigation();
  const { data: games = [], isPending, isError } = useGames();
  const activeView = useProductStore((state) => state.activeView);
  const selectedGameId = useProductStore((state) => state.selectedGameId);
  const returnView = useProductStore((state) => state.returnView);
  const returnFocusId = useProductStore((state) => state.returnFocusId);
  const setView = useProductStore((state) => state.setView);
  const openDetails = useProductStore((state) => state.openDetails);
  const selectedGame = games.find((game) => game.id === selectedGameId);
  useEffect(() => {
    if (activeView === "details" && selectedGameId && !selectedGame)
      setView(returnView);
  }, [activeView, returnView, selectedGame, selectedGameId, setView]);

  useEffect(() => {
    if (activeView === "details") {
      engine.prepareScopeOpen("details", returnFocusId ?? undefined);
      return;
    }
    if (
      returnFocusId &&
      document.querySelector(`[data-focus-id="${returnFocusId}"]`)
    ) {
      engine.focus(returnFocusId);
    }
  }, [activeView, engine, returnFocusId]);

  useEffect(() => {
    markPerformance("view-active");
    markPerformance("main-content-updated");
  }, [activeView]);

  const navigate = (view: ProductView) => {
    markPerformance("view-requested");
    if (view !== "library") engine.cancelPendingVirtualFocus("view-change");
    setView(view);
  };
  const handleOpen = (game: Game) => {
    markPerformance("view-requested");
    const openerFocusId = engine.getActiveFocusId();
    engine.prepareScopeOpen("details", openerFocusId ?? undefined);
    openDetails(game.id, activeView, openerFocusId);
  };

  return (
    <div className="app-shell">
      <BackgroundView games={games} fallbackGameId={selectedGameId} />
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
          {!isPending && !isError && (
            <ViewTransition view={activeView}>
              {activeView === "home" && (
                <HomeView games={games} onOpen={handleOpen} />
              )}
              {activeView === "library" && <LibraryView onOpen={handleOpen} />}
              {activeView === "details" && <DetailsView game={selectedGame} />}
            </ViewTransition>
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
