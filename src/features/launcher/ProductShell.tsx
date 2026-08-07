import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useGames } from "../catalog/catalog-query";
import type { Game } from "../catalog/game-types";
import { useProductStore, type ProductView } from "../../stores/product-store";
import { useNavigation } from "../../ui/navigation/navigation-context";
import { Focusable } from "../../ui/navigation/focus/Focusable";
import { NavigationTabs } from "../../ui/navigation/layouts/NavigationTabs";
import { NavigationDebugOverlay } from "../../ui/navigation/debug/NavigationDebugOverlay";
import { PerformanceOverlay } from "../../ui/performance/PerformanceOverlay";
import { BackgroundView } from "../../ui/background/BackgroundView";
import { DetailsView } from "./DetailsView";
import { HomeView, HOME_SCREEN_DEFINITION } from "./HomeView";
import { LibraryView } from "./LibraryView";
import { ViewTransition } from "../../ui/motion/ViewTransition";
import { recordRender } from "../../ui/performance/performance-counters";
import { markPerformance } from "../../ui/performance/performance-marks";
import { WeatherWidget } from "../../ui/weather/WeatherWidget";
import { ScreenNavigationAdapter } from "../../ui/navigation/screen/ScreenNavigationAdapter";
import {
  PrimaryScreenNavigator,
  type PrimaryScreenDefinition,
} from "../../ui/navigation/screen/primary-screen-navigator";
import { navigationRuntimeTrace } from "../../ui/navigation/debug/navigation-runtime-trace";
import {
  SettingsView,
  SETTINGS_SCREEN_DEFINITION,
} from "../settings/SettingsView";
import { providerSettingsService } from "../settings/provider-settings-service";
import type { SettingsLevel, SteamProfile } from "../settings/settings-types";
import { GameSessionScreen } from "../game-session/GameSessionScreen";

type PrimaryProductView = Exclude<ProductView, "details">;

const PRIMARY_SCREEN_SEQUENCE: readonly PrimaryScreenDefinition<PrimaryProductView>[] =
  [
    { id: "home", initialFocusId: "main-nav-home" },
    { id: "library", initialFocusId: "library-filter-all" },
    { id: "settings", initialFocusId: "settings-integrations" },
  ];

export function ProductShell() {
  recordRender("app-shell");
  const { engine, inputManager, registry } = useNavigation();
  const { data: games = [], isPending, isError } = useGames();
  const activeView = useProductStore((state) => state.activeView);
  const selectedGameId = useProductStore((state) => state.selectedGameId);
  const returnView = useProductStore((state) => state.returnView);
  const viewTransitionId = useProductStore((state) => state.viewTransitionId);
  const setView = useProductStore((state) => state.setView);
  const openDetails = useProductStore((state) => state.openDetails);
  const [settingsLevel, setSettingsLevel] = useState<SettingsLevel>("settings");
  const [steamProfile, setSteamProfile] = useState<SteamProfile | null>(null);
  const settingsBackRef = useRef<(() => boolean) | null>(null);
  const selectedGame = games.find((game) => game.id === selectedGameId);
  const primaryFocusByScreen = useMemo(
    () => new Map<PrimaryProductView, string>(),
    [],
  );
  const primaryTransitionRef = useMemo(
    () => ({
      target: null as PrimaryProductView | null,
      fromMainNavigation: false,
    }),
    [],
  );
  const homeEntryFocusId = `home-continue-${
    games.find((game) => game.status === "playing")?.id ?? "empty"
  }`;
  useEffect(() => {
    let disposed = false;
    if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) {
      return () => {
        disposed = true;
      };
    }

    void providerSettingsService
      .getSteamProfile()
      .then((profile) => {
        if (!disposed) setSteamProfile(profile);
      })
      .catch(() => {
        if (!disposed) setSteamProfile(null);
      });

    return () => {
      disposed = true;
    };
  }, []);
  useEffect(() => {
    if (activeView === "details" && selectedGameId && !selectedGame)
      setView(returnView);
  }, [activeView, returnView, selectedGame, selectedGameId, setView]);

  useLayoutEffect(() => {
    if (activeView === "details") return;
    const activeFocusId = engine.getActiveFocusId();
    const activeEntry = activeFocusId
      ? engine.registry.get(activeFocusId)
      : undefined;
    const pendingTransition =
      primaryTransitionRef.target === activeView ? primaryTransitionRef : null;
    const targetFocusId = pendingTransition
      ? pendingTransition.fromMainNavigation
        ? activeView === "settings"
          ? undefined
          : activeView === "home"
            ? "main-nav-home"
            : "main-nav-library"
        : (primaryFocusByScreen.get(activeView) ??
          PRIMARY_SCREEN_SEQUENCE.find((screen) => screen.id === activeView)
            ?.initialFocusId)
      : activeEntry?.navigationRegion?.regionId === "main-navigation"
        ? activeView === "home"
          ? "main-nav-home"
          : activeView === "library"
            ? "main-nav-library"
            : "main-nav-settings"
        : undefined;
    if (targetFocusId && activeFocusId !== targetFocusId) {
      engine.focus(targetFocusId);
    }
    if (pendingTransition) primaryTransitionRef.target = null;
  }, [activeView, engine, primaryFocusByScreen, primaryTransitionRef]);

  useEffect(() => {
    navigationRuntimeTrace.record("route_transition", {
      details: { view: activeView, transitionId: viewTransitionId },
    });
    markPerformance("view-active");
    markPerformance("main-content-updated");
  }, [activeView, viewTransitionId]);

  useLayoutEffect(() => {
    if (activeView === "library") engine.notifyRouteActive("product-shell");
  }, [activeView, engine]);

  const navigate = useCallback(
    (view: PrimaryProductView) => {
      const currentView = useProductStore.getState().activeView;
      const activeFocusId = engine.getActiveFocusId();
      const activeEntry = activeFocusId
        ? registry.get(activeFocusId)
        : undefined;
      const fromMainNavigation =
        activeEntry?.navigationRegion?.regionId === "main-navigation";
      if (
        activeFocusId &&
        !fromMainNavigation &&
        (currentView === "home" || currentView === "library")
      ) {
        primaryFocusByScreen.set(currentView, activeFocusId);
      }
      primaryTransitionRef.target = view;
      primaryTransitionRef.fromMainNavigation = fromMainNavigation;
      markPerformance("view-requested");
      engine.cancelPendingHierarchyFocus();
      if (view !== "library") engine.cancelPendingVirtualFocus("view-change");
      setView(view);
      if (view === "settings") setSettingsLevel("settings");
    },
    [engine, primaryFocusByScreen, primaryTransitionRef, registry, setView],
  );
  const primaryScreenNavigator = useMemo(
    () =>
      new PrimaryScreenNavigator<PrimaryProductView>({
        screens: PRIMARY_SCREEN_SEQUENCE,
        getCurrentScreen: () => useProductStore.getState().activeView,
        getBlockReason: () => engine.getPrimaryNavigationBlockReason(),
        onTransitionRequest: (targetScreen) => navigate(targetScreen),
      }),
    [engine, navigate],
  );
  useEffect(
    () =>
      inputManager.setSemanticActionHandler((action, inputMode) =>
        primaryScreenNavigator.handle(action, inputMode),
      ),
    [inputManager, primaryScreenNavigator],
  );
  const handleOpen = (game: Game) => {
    markPerformance("view-requested");
    const openerFocusId = engine.getActiveFocusId();
    navigationRuntimeTrace.setOpener(game.id, openerFocusId);
    navigationRuntimeTrace.record("details_open", {
      details: {
        openerGameId: game.id,
        openerFocusId,
        returnView: activeView,
      },
    });
    engine.prepareScopeOpen("details", openerFocusId ?? undefined);
    openDetails(game.id, activeView, openerFocusId);
  };
  const handleCloseDetails = () => {
    navigationRuntimeTrace.record("details_close", {
      details: {
        returnView,
        transitionId: viewTransitionId,
      },
    });
    engine.requestScopeRestore(
      "details",
      "product-shell",
      `details-to-${returnView}-${viewTransitionId}`,
    );
    useProductStore.getState().closeDetails();
  };

  const settingsScreenDefinition = useMemo(
    () => ({
      ...SETTINGS_SCREEN_DEFINITION,
      onBack: () => settingsBackRef.current?.() ?? true,
    }),
    [],
  );
  const screenDefinition =
    activeView === "settings"
      ? settingsScreenDefinition
      : HOME_SCREEN_DEFINITION;
  const activeRootScopeId = screenDefinition.rootScope.scopeId;

  return (
    <div
      className={`app-shell${activeView === "home" ? " app-shell-home" : ""}${activeView === "details" ? " app-shell-details" : ""}`}
    >
      <BackgroundView games={games} fallbackGameId={selectedGameId} />
      <ScreenNavigationAdapter
        definition={screenDefinition}
        active={activeView === "settings" || activeView === "home"}
      >
        <header className="app-header">
          <div className="brand-lockup">
            <span className="brand-mark">L</span>
            <span>LumaDeck</span>
          </div>
          <NavigationTabs groupId="main-navigation" className="primary-nav">
            <Focusable
              focusId="main-nav-home"
              scopeId={activeRootScopeId}
              className="shell-nav-button"
              ariaCurrent={activeView === "home" ? "page" : false}
              navigationRegion={{
                regionId: "main-navigation",
                childRegionId: "home-content",
                entryFocusId: homeEntryFocusId,
              }}
              onConfirm={() => navigate("home")}
            >
              Home
            </Focusable>
            <Focusable
              focusId="main-nav-library"
              scopeId={activeRootScopeId}
              className="shell-nav-button"
              ariaCurrent={activeView === "library" ? "page" : false}
              navigationRegion={{
                regionId: "main-navigation",
                childRegionId: "library-content",
                entryFocusId: "library-game-001",
              }}
              onConfirm={() => navigate("library")}
            >
              Library
            </Focusable>
            <Focusable
              focusId="main-nav-settings"
              scopeId={activeRootScopeId}
              className="shell-nav-button"
              ariaCurrent={activeView === "settings" ? "page" : false}
              navigationRegion={{
                regionId: "main-navigation",
                childRegionId: "settings-content",
                entryFocusId: "settings-integrations",
              }}
              onConfirm={() => navigate("settings")}
            >
              Settings
            </Focusable>
          </NavigationTabs>
          <div className="shell-utilities">
            <span className="shell-catalog-status">
              LOCAL CATALOG · {games.length}
            </span>
            <span className="shell-utility-icon" aria-label="Wi-Fi connected">
              ◔
            </span>
            <span
              className="shell-utility-icon"
              aria-label="Controller connected"
            >
              ♧
            </span>
            <WeatherWidget />
            <span
              className="shell-avatar"
              aria-label={
                steamProfile
                  ? `Steam profile: ${steamProfile.personaName}`
                  : "Profile"
              }
            >
              {steamProfile?.avatarUrl ? (
                <img
                  className="shell-avatar-image"
                  src={steamProfile.avatarUrl}
                  alt=""
                  draggable={false}
                />
              ) : (
                "L"
              )}
              <span className="shell-avatar-status" aria-hidden="true" />
            </span>
          </div>
          <span className="shell-status">LOCAL CATALOG · {games.length}</span>
        </header>
        <main className="app-shell-main">
          {activeView !== "settings" && isPending && (
            <p className="loading-state">Preparing your library…</p>
          )}
          {activeView !== "settings" && isError && (
            <p className="empty-state">Could not load the local catalog.</p>
          )}
          {activeView === "settings" && (
            <SettingsView
              level={settingsLevel}
              onLevelChange={setSettingsLevel}
              onClose={() => navigate("library")}
              backHandlerRef={settingsBackRef}
            />
          )}
          {activeView !== "settings" && !isPending && !isError && (
            <ViewTransition view={activeView}>
              {activeView === "home" && (
                <HomeView
                  games={games}
                  onOpen={handleOpen}
                  onViewLibrary={() => navigate("library")}
                />
              )}
              {activeView === "library" && <LibraryView onOpen={handleOpen} />}
              {activeView === "details" && (
                <DetailsView
                  game={selectedGame}
                  games={games}
                  onClose={handleCloseDetails}
                />
              )}
            </ViewTransition>
          )}
        </main>
        <footer className="app-footer">
          <div className="footer-controls">
            {activeView === "details" ? (
              <>
                <span>
                  <strong>✣</strong> Navigate
                </span>
                <span>
                  <strong className="footer-a-button">A</strong> Select
                </span>
                <span>
                  <strong className="footer-b-button">B</strong> Back
                </span>
                <span>
                  <strong className="footer-trigger-button">LB</strong>
                  <strong className="footer-trigger-button">RB</strong> Change
                  Tab
                </span>
                <span>
                  <strong className="footer-trigger-button">LT</strong>
                  <strong className="footer-trigger-button">RT</strong> Change
                  View
                </span>
              </>
            ) : (
              <>
                <span>
                  <strong>✣</strong> Navigate
                </span>
                <span>
                  <strong className="footer-a-button">A</strong> Select
                </span>
                <span>
                  <strong className="footer-b-button">B</strong> Back
                </span>
                <span>
                  <strong className="footer-trigger-button">LT</strong>
                  <strong className="footer-trigger-button">RT</strong> Change
                  View
                </span>
              </>
            )}
          </div>
          <span>↑ ↓ ← → navigate</span>
          <span>Enter / Space select</span>
          <span>Esc back</span>
          <span className="footer-spacer" />
          <span className="footer-clock">{formatClock()}</span>
          <span>Local-first launcher slice</span>
        </footer>
      </ScreenNavigationAdapter>
      <GameSessionScreen games={games} />
      <NavigationDebugOverlay />
      <PerformanceOverlay />
    </div>
  );
}

function formatClock(): string {
  return new Date().toLocaleTimeString(undefined, {
    hour: "numeric",
    minute: "2-digit",
  });
}
