import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  useMemo,
} from "react";
import { createPortal } from "react-dom";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import type {
  Game,
  GameDetails,
  HltbGameData,
  SteamGameDetails,
  SteamGameMetrics,
} from "../catalog/game-types";
import { getVisibleGames } from "../catalog/game-visibility";
import { fetchGameDetails } from "../catalog/catalog-query";
import { getGameBackgroundUrl } from "../catalog/game-media";
import { toPlainText } from "../catalog/text-utils";
import { useProductStore } from "../../stores/product-store";
import { Focusable } from "../../ui/navigation/focus/Focusable";
import { FocusScope } from "../../ui/navigation/focus/FocusScope";
import { useNavigation } from "../../ui/navigation/navigation-context";
import type { NavigationAction } from "../../ui/navigation/core/navigation-types";
import {
  NavigationTab,
  NavigationTabs,
} from "../../ui/navigation/layouts/NavigationTabs";
import { NavigationContent } from "../../ui/navigation/layouts/NavigationContent";
import { NavigationGrid } from "../../ui/navigation/layouts/NavigationGrid";
import {
  launchBoxErrorMessage,
  providerSettingsService,
} from "../settings/provider-settings-service";
import { SteamTrailer } from "./SteamTrailer";
import { formatHltbDuration } from "./hltb-format";
import { ArtworkModifierView } from "../artwork/ArtworkModifierView";
import { ActivityView } from "../activity/ActivityView";
import { AchievementsView } from "../achievements/AchievementsView";
import { NewsView } from "../news/NewsView";
import { ReviewsView } from "../reviews/ReviewsView";
import { newsService } from "../news/news-service";
import {
  gameSessionService,
  gameSessionErrorMessage,
} from "../game-session/game-session-service";
import { useGameSessionStore } from "../game-session/game-session-store";
import {
  displayProfileErrorMessage,
  displayProfileService,
  formatDisplayRefreshRate,
  formatDisplayResolution,
  type DisplayMode,
  type DisplayProfile,
  type RtxHdrPreset,
} from "./display-profile-service";
import {
  frameGenerationErrorMessage,
  frameGenerationLabel,
  frameGenerationService,
  type FrameGenerationProfile,
} from "./frame-generation-service";
import { DlcView } from "../dlc/DlcView";
import { RelatedGamesView } from "../related/RelatedGamesView";
import {
  ScreenshotViewer,
  type ScreenshotViewerOrigin,
} from "./ScreenshotViewer";
import { GameCapabilitiesPanel } from "../game-capabilities/GameCapabilitiesPanel";
import { hardwareCapabilitiesService } from "../graphics-profile/hardware-capabilities-service";
import { MediaImage } from "../../ui/performance/MediaImage";
import { recordMediaTiming } from "../../ui/performance/media-timing";
import { DetailsTabContent } from "./DetailsTabContent";
import {
  getDetailsReadiness,
  shouldShowEmptyScreenshots,
} from "./details-readiness";
import {
  DETAILS_TAB_ORDER,
  type DetailsSection,
} from "./details-view-contract";

const CONTEXT_MENU_SCOPE_ID = "details-context-menu";

export function DetailsView({
  game: initialGame,
  games,
  onClose,
}: {
  game: Game | undefined;
  games: readonly Game[];
  onClose?: () => void;
}) {
  const closeDetails = useProductStore((state) => state.closeDetails);
  const queryClient = useQueryClient();
  const mediaQuery = useQuery({
    queryKey: ["game-details", initialGame?.id],
    queryFn: async (): Promise<Game> => {
      if (!initialGame) throw new Error("Game not found");
      const queryStartedAt = performance.now();
      recordMediaTiming("DETAILS_QUERY_FETCH_START", {
        gameId: initialGame.id,
        type: "screenshot",
        path: initialGame.screenshots[0],
        detail: JSON.stringify({ source: "get_library_game" }),
      });
      try {
        const resolvedGame = await fetchGameDetails(initialGame);
        recordMediaTiming("DETAILS_QUERY_FETCH_END", {
          gameId: resolvedGame.id,
          type: "screenshot",
          path: resolvedGame.screenshots[0],
          durationMs: performance.now() - queryStartedAt,
          detail: JSON.stringify({
            source: "get_library_game",
            screenshots: resolvedGame.screenshots.length,
          }),
        });
        return resolvedGame;
      } catch (error) {
        recordMediaTiming("DETAILS_QUERY_FETCH_END", {
          gameId: initialGame.id,
          type: "screenshot",
          path: initialGame.screenshots[0],
          durationMs: performance.now() - queryStartedAt,
          detail: JSON.stringify({
            source: "get_library_game",
            error: String(error),
          }),
        });
        throw error;
      }
    },
    enabled: Boolean(initialGame?.id),
    staleTime: Infinity,
    refetchOnWindowFocus: false,
    retry: false,
  });
  // Details is not allowed to render the catalogue snapshot while its
  // persisted details row is still hydrating. ProductShell normally opens
  // this view only after the same query and media are ready, but keeping the
  // guard here makes the invariant local to the view as well.
  const game = mediaQuery.data;
  const { engine } = useNavigation();
  const [message, setMessage] = useState("");
  const [menuOpen, setMenuOpen] = useState(false);
  const menuButtonRef = useRef<HTMLButtonElement | null>(null);
  const [contextMenuPosition, setContextMenuPosition] = useState<{
    top: number;
    left: number;
    transform: string;
  } | null>(null);
  const [displayProfileMenu, setDisplayProfileMenu] = useState<
    "root" | "profile" | "resolution" | "refresh" | "hdr" | "frame-generation"
  >("root");
  const [displayProfile, setDisplayProfile] = useState<DisplayProfile | null>(
    null,
  );
  const [displayModes, setDisplayModes] = useState<DisplayMode[]>([]);
  const [isSavingDisplayProfile, setIsSavingDisplayProfile] = useState(false);
  const [frameGenerationProfile, setFrameGenerationProfile] =
    useState<FrameGenerationProfile | null>(null);
  const [isSavingFrameGeneration, setIsSavingFrameGeneration] = useState(false);
  const hardwareQuery = useQuery({
    queryKey: ["hardware-capabilities"],
    queryFn: () => hardwareCapabilitiesService.get(),
    staleTime: Infinity,
    refetchOnWindowFocus: false,
    retry: false,
  });
  const rtxHdrAvailable =
    hardwareQuery.data?.vendor === "NVIDIA" &&
    hardwareQuery.data.featureSupport.supportsDlss === "SUPPORTED";
  const [isRefreshingMetadata, setIsRefreshingMetadata] = useState(false);
  const [isDownloadingMedia, setIsDownloadingMedia] = useState(false);
  const [isUpdatingFavorite, setIsUpdatingFavorite] = useState(false);
  const [isUpdatingHidden, setIsUpdatingHidden] = useState(false);
  const [artworkModifierOpen, setArtworkModifierOpen] = useState(false);
  const [activeSection, setActiveSection] = useState<DetailsSection>("summary");
  const [screenshotViewerOpen, setScreenshotViewerOpen] = useState(false);
  const [screenshotViewerInitialIndex, setScreenshotViewerInitialIndex] =
    useState(0);
  const [screenshotViewerOrigin, setScreenshotViewerOrigin] =
    useState<ScreenshotViewerOrigin | null>(null);
  const [detailsContentDirection, setDetailsContentDirection] = useState<
    "forward" | "backward"
  >("forward");
  const artworkWasOpenRef = useRef(false);
  const metricsRefreshTimerRef = useRef<number | null>(null);
  const [liveMetrics, setLiveMetrics] = useState<SteamGameMetrics | null>(null);
  const [isRefreshingMetrics, setIsRefreshingMetrics] = useState(false);
  const handleClose = onClose ?? closeDetails;
  const openingGameRef = useRef(initialGame);
  const detailsLifecycleState = useRef({ version: 0 }).current;
  const openingQueryStateRef = useRef({
    dataPresent: Boolean(mediaQuery.data),
    status: mediaQuery.status,
    fetchStatus: mediaQuery.fetchStatus,
    isStale: mediaQuery.isStale,
  });

  useEffect(() => {
    const openingGame = openingGameRef.current;
    if (!openingGame?.id) return;
    const lifecycleVersion = ++detailsLifecycleState.version;
    const openingQueryState = openingQueryStateRef.current;
    const queryKey = ["game-details", openingGame.id] as const;
    const cachedQuery = queryClient.getQueryCache().find({ queryKey });
    recordMediaTiming("DETAILS_OPEN", {
      gameId: openingGame.id,
      type: "screenshot",
      path: openingGame.screenshots[0],
      detail: JSON.stringify({
        queryKey,
        source: openingQueryState.dataPresent ? "query-cache" : "initial-game",
        queryExists: Boolean(cachedQuery),
        queryStatus: cachedQuery?.state.status ?? openingQueryState.status,
        fetchStatus:
          cachedQuery?.state.fetchStatus ?? openingQueryState.fetchStatus,
        dataPresent: Boolean(cachedQuery?.state.data),
        dataUpdatedAt: cachedQuery?.state.dataUpdatedAt ?? 0,
        isInvalidated: cachedQuery?.state.isInvalidated ?? false,
        isStale: cachedQuery?.isStale() ?? openingQueryState.isStale,
        staleTime: "Infinity",
        gcTime: cachedQuery?.gcTime ?? null,
        screenshots: openingGame.screenshots.length,
      }),
    });
    return () => {
      queueMicrotask(() => {
        if (detailsLifecycleState.version !== lifecycleVersion) return;
        const leavingQuery = queryClient.getQueryCache().find({ queryKey });
        recordMediaTiming("DETAILS_LEAVE", {
          gameId: openingGame.id,
          type: "screenshot",
          path: openingGame.screenshots[0],
          detail: JSON.stringify({
            queryExists: Boolean(leavingQuery),
            dataPresent: Boolean(leavingQuery?.state.data),
            dataUpdatedAt: leavingQuery?.state.dataUpdatedAt ?? 0,
            isInvalidated: leavingQuery?.state.isInvalidated ?? false,
            isStale: leavingQuery?.isStale() ?? true,
            gcTime: leavingQuery?.gcTime ?? null,
            screenshots: openingGame.screenshots.length,
          }),
        });
      });
    };
  }, [queryClient, detailsLifecycleState]);

  const renderedScreenshotUrl = game?.screenshots[0];
  const renderedScreenshotCount = game?.screenshots.length ?? 0;
  const queryDataPresent = Boolean(mediaQuery.data);

  useEffect(() => {
    if (!initialGame?.id) return;
    const queryKey = ["game-details", initialGame.id] as const;
    const cachedQuery = queryClient.getQueryCache().find({ queryKey });
    recordMediaTiming("DETAILS_QUERY_STATE", {
      gameId: initialGame.id,
      type: "screenshot",
      path: renderedScreenshotUrl,
      detail: JSON.stringify({
        queryKey,
        queryStatus: cachedQuery?.state.status ?? mediaQuery.status,
        fetchStatus: cachedQuery?.state.fetchStatus ?? mediaQuery.fetchStatus,
        dataPresent: Boolean(cachedQuery?.state.data),
        dataUpdatedAt: cachedQuery?.state.dataUpdatedAt ?? 0,
        isInvalidated: cachedQuery?.state.isInvalidated ?? false,
        isStale: cachedQuery?.isStale() ?? mediaQuery.isStale,
        staleTime: "Infinity",
        gcTime: cachedQuery?.gcTime ?? null,
        screenshots: renderedScreenshotCount,
        dataSource: queryDataPresent ? "query-cache" : "initial-game",
      }),
    });
  }, [
    initialGame?.id,
    queryDataPresent,
    renderedScreenshotCount,
    renderedScreenshotUrl,
    mediaQuery.data,
    mediaQuery.dataUpdatedAt,
    mediaQuery.fetchStatus,
    mediaQuery.isStale,
    mediaQuery.status,
    queryClient,
  ]);

  const applyLiveMetrics = useCallback(
    (metrics: SteamGameMetrics) => {
      setLiveMetrics(metrics);
      queryClient.setQueryData<Game[]>(["games"], (games) =>
        games?.map((candidate) =>
          candidate.id === game?.id
            ? {
                ...candidate,
                playtimeMinutes: metrics.totalPlaytimeMinutes,
                lastPlayedAt: metrics.lastPlayedAt,
                progress: metrics.progress,
                achievements:
                  metrics.achievementTotal !== null ||
                  metrics.achievementUnlocked !== null
                    ? {
                        total: metrics.achievementTotal,
                        unlocked: metrics.achievementUnlocked,
                        progress:
                          metrics.achievementTotal &&
                          metrics.achievementTotal > 0 &&
                          metrics.achievementUnlocked !== null
                            ? (metrics.achievementUnlocked /
                                metrics.achievementTotal) *
                              100
                            : null,
                      }
                    : candidate.achievements,
              }
            : candidate,
        ),
      );
    },
    [game?.id, queryClient],
  );

  const loadLiveMetrics = useCallback(
    async (showAnimation = false, refreshAchievements = false) => {
      if (!game?.id) return;
      if (metricsRefreshTimerRef.current !== null) {
        window.clearTimeout(metricsRefreshTimerRef.current);
        metricsRefreshTimerRef.current = null;
      }
      const startedAt = performance.now();
      if (showAnimation) setIsRefreshingMetrics(true);
      try {
        const metrics = await providerSettingsService.refreshSteamGameMetrics(
          game.id,
        );
        applyLiveMetrics(metrics);
      } catch {
        setLiveMetrics(null);
      } finally {
        if (refreshAchievements) {
          try {
            const metrics =
              await providerSettingsService.refreshSteamGameAchievements(
                game.id,
              );
            applyLiveMetrics(metrics);
          } catch {
            // Keep the metrics refresh useful when Steam achievements are unavailable.
          }
        }
        if (showAnimation) {
          const remaining = Math.max(0, 650 - (performance.now() - startedAt));
          metricsRefreshTimerRef.current = window.setTimeout(() => {
            metricsRefreshTimerRef.current = null;
            setIsRefreshingMetrics(false);
          }, remaining);
        }
      }
    },
    [applyLiveMetrics, game?.id],
  );

  useLayoutEffect(() => {
    if (artworkModifierOpen) {
      artworkWasOpenRef.current = true;
      return;
    }
    if (!artworkWasOpenRef.current) return;
    artworkWasOpenRef.current = false;
    if (
      engine.getActiveScopeId() === "details" &&
      (engine.getActiveFocusId() === null ||
        engine.getActiveFocusId() === "details-play")
    ) {
      engine.focus("details-back");
    }
  }, [artworkModifierOpen, engine]);

  useEffect(() => {
    setMenuOpen(false);
    setDisplayProfileMenu("root");
    setDisplayProfile(null);
    setDisplayModes([]);
    setIsSavingDisplayProfile(false);
    setFrameGenerationProfile(null);
    setIsSavingFrameGeneration(false);
    setIsRefreshingMetadata(false);
    setIsDownloadingMedia(false);
    setIsUpdatingFavorite(false);
    setIsUpdatingHidden(false);
    setArtworkModifierOpen(false);
    setActiveSection("summary");
    setScreenshotViewerOpen(false);
    setScreenshotViewerInitialIndex(0);
    setScreenshotViewerOrigin(null);
    setDetailsContentDirection("forward");
    setLiveMetrics(null);
    setIsRefreshingMetrics(false);
    setMessage("");
  }, [game?.id]);

  useEffect(() => {
    if (!game?.id) return;
    let disposed = false;
    void Promise.all([
      displayProfileService.getProfile(game.id),
      displayProfileService.getModes(),
      frameGenerationService.getProfile(game.id),
    ])
      .then(([profile, modes, frameProfile]) => {
        if (disposed) return;
        setDisplayProfile(profile);
        setDisplayModes(modes);
        setFrameGenerationProfile(frameProfile);
      })
      .catch((error: unknown) => {
        if (!disposed) setMessage(displayProfileErrorMessage(error));
      });
    return () => {
      disposed = true;
    };
  }, [game?.id]);

  useEffect(() => {
    void loadLiveMetrics();
  }, [loadLiveMetrics]);

  useEffect(() => {
    if (!game?.id || game.source !== "emulator") return;
    let disposed = false;
    void providerSettingsService
      .downloadLaunchBoxScreenshots(game.id)
      .then(() => {
        if (!disposed) {
          void queryClient.invalidateQueries({ queryKey: ["games"] });
        }
      })
      .catch(() => {
        // Details stays usable when LaunchBox media is unavailable or offline.
      });
    return () => {
      disposed = true;
    };
  }, [game?.id, game?.source, queryClient]);

  useEffect(() => {
    const gameId = game?.id;
    if (
      !gameId ||
      typeof window === "undefined" ||
      !("__TAURI_INTERNALS__" in window)
    ) {
      return;
    }
    let disposed = false;
    void newsService
      .refresh(gameId, false)
      .then(() => {
        if (disposed) return;
        void queryClient.invalidateQueries({
          queryKey: ["news-feed", gameId],
        });
      })
      .catch(() => {
        // News remains available from the persistent cache when Steam is unavailable.
      });
    return () => {
      disposed = true;
    };
  }, [game?.id, queryClient]);

  useEffect(() => {
    if (!game?.id) return;
    let sessionWasActive = false;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void gameSessionService
      .subscribe((status) => {
        if (disposed || status.gameId !== game.id) return;
        if (
          status.state === "preparing" ||
          status.state === "launching" ||
          status.state === "running" ||
          status.state === "finishing"
        ) {
          sessionWasActive = true;
          return;
        }
        if (status.state === "idle" && sessionWasActive) {
          sessionWasActive = false;
          void loadLiveMetrics(true, true);
        }
      })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      });
    return () => {
      disposed = true;
      unlisten?.();
      if (metricsRefreshTimerRef.current !== null) {
        window.clearTimeout(metricsRefreshTimerRef.current);
        metricsRefreshTimerRef.current = null;
      }
    };
  }, [game?.id, loadLiveMetrics]);

  useEffect(() => {
    if (menuOpen && displayProfileMenu === "root") {
      engine.focus("details-display-profile");
      return;
    }
    if (menuOpen && displayProfileMenu === "profile") {
      engine.focus("details-display-mode");
      return;
    }
    if (menuOpen && displayProfileMenu === "resolution") {
      engine.focus("details-display-resolution-auto");
      return;
    }
    if (menuOpen && displayProfileMenu === "refresh") {
      const firstRefresh = displayModes
        .filter(
          (mode) =>
            mode.width === displayProfile?.width &&
            mode.height === displayProfile?.height,
        )
        .map((mode) => mode.refreshRate)
        .sort((left, right) => left - right)[0];
      if (firstRefresh) engine.focus(`details-display-refresh-${firstRefresh}`);
      return;
    }
    if (menuOpen && displayProfileMenu === "hdr") {
      engine.focus("details-display-hdr-system");
      return;
    }
    if (menuOpen && displayProfileMenu === "frame-generation") {
      engine.focus("details-frame-generation-off");
      return;
    }
    if (!menuOpen && engine.getActiveFocusId() === "details-display-profile") {
      engine.focus("details-back");
    }
  }, [
    displayModes,
    displayProfile?.height,
    displayProfile?.width,
    displayProfileMenu,
    engine,
    menuOpen,
  ]);

  useLayoutEffect(() => {
    if (!menuOpen) {
      setContextMenuPosition(null);
      return;
    }
    const button = menuButtonRef.current;
    if (!button) return;
    const rect = button.getBoundingClientRect();
    const isNarrow = window.innerWidth <= 620;
    setContextMenuPosition({
      top: isNarrow ? rect.bottom + 12 : rect.top + rect.height / 2,
      left: isNarrow
        ? Math.max(16, window.innerWidth - 16 - 260)
        : rect.right + 12,
      transform: isNarrow ? "none" : "translateY(-50%)",
    });
  }, [displayProfileMenu, menuOpen]);

  const toggleMenu = () => {
    if (!isRefreshingMetadata && !isDownloadingMedia) {
      if (menuOpen) {
        setDisplayProfileMenu("root");
        setMenuOpen(false);
        return;
      }
      engine.prepareScopeOpen(CONTEXT_MENU_SCOPE_ID, "details-back");
      setDisplayProfileMenu("root");
      setMenuOpen(true);
    }
  };

  const saveProfile = async (next: DisplayProfile) => {
    if (isSavingDisplayProfile) return;
    setIsSavingDisplayProfile(true);
    try {
      const saved = await displayProfileService.saveProfile(next);
      setDisplayProfile(saved);
      setMessage("Perfil de pantalla guardado.");
    } catch (error: unknown) {
      setMessage(displayProfileErrorMessage(error));
    } finally {
      setIsSavingDisplayProfile(false);
    }
  };

  const openDisplayProfile = () => {
    if (!displayProfile) {
      setMessage("Cargando los modos de pantalla disponibles...");
      return;
    }
    setDisplayProfileMenu("profile");
  };

  const toggleDisplayProfileMode = async () => {
    if (!displayProfile || isSavingDisplayProfile) return;
    const hasCustomMode =
      displayProfile.resolutionMode === "CUSTOM" ||
      displayProfile.refreshRateMode === "CUSTOM";
    if (hasCustomMode) {
      await saveProfile({
        ...displayProfile,
        enabled: false,
        resolutionMode: "SYSTEM",
        refreshRateMode: "SYSTEM",
      });
      return;
    }
    const currentMode = await displayProfileService
      .getCurrentMode()
      .catch(() => null);
    const fallbackMode = currentMode ?? displayModes[0];
    if (!fallbackMode) {
      setMessage("No hay modos de pantalla disponibles para elegir.");
      return;
    }
    await saveProfile({
      ...displayProfile,
      enabled: true,
      resolutionMode: "CUSTOM",
      refreshRateMode: "CUSTOM",
      displayId: fallbackMode.displayId,
      deviceName: fallbackMode.deviceName,
      width: fallbackMode.width,
      height: fallbackMode.height,
      refreshRate: fallbackMode.refreshRate,
      restoreOnExit: true,
    });
  };

  const chooseDisplayResolution = async (mode: DisplayMode | null) => {
    if (!displayProfile || !mode) return;
    const matchingRefreshRates = displayModes
      .filter(
        (candidate) =>
          candidate.width === mode.width && candidate.height === mode.height,
      )
      .map((candidate) => candidate.refreshRate);
    const refreshRate = matchingRefreshRates.includes(
      displayProfile.refreshRate ?? -1,
    )
      ? displayProfile.refreshRate
      : (matchingRefreshRates[0] ?? mode.refreshRate);
    await saveProfile({
      ...displayProfile,
      enabled: true,
      resolutionMode: "CUSTOM",
      displayId: mode.displayId,
      deviceName: mode.deviceName,
      width: mode.width,
      height: mode.height,
      refreshRate,
    });
    setDisplayProfileMenu("profile");
  };

  const chooseDisplayRefreshRate = async (refreshRate: number) => {
    if (!displayProfile) return;
    await saveProfile({
      ...displayProfile,
      enabled: true,
      refreshRateMode: "CUSTOM",
      refreshRate,
    });
    setDisplayProfileMenu("profile");
  };

  const chooseDisplayHdr = async (hdrMode: DisplayProfile["hdrMode"]) => {
    if (!displayProfile) return;
    const currentMode =
      hdrMode === "SYSTEM"
        ? null
        : await displayProfileService.getCurrentMode().catch(() => null);
    await saveProfile({
      ...displayProfile,
      hdrMode,
      rtxHdrPreset: null,
      displayId: currentMode?.displayId ?? displayProfile.displayId,
      deviceName: currentMode?.deviceName ?? displayProfile.deviceName,
    });
    setDisplayProfileMenu("profile");
  };

  const chooseRtxHdr = async (preset: RtxHdrPreset) => {
    if (!displayProfile) return;
    const currentMode = await displayProfileService
      .getCurrentMode()
      .catch(() => null);
    await saveProfile({
      ...displayProfile,
      enabled: true,
      hdrMode: "ON",
      rtxHdrPreset: preset,
      rtxHdrPeakNits: displayProfile.rtxHdrPeakNits || 800,
      displayId: currentMode?.displayId ?? displayProfile.displayId,
      deviceName: currentMode?.deviceName ?? displayProfile.deviceName,
    });
    setDisplayProfileMenu("profile");
  };

  const resetDisplayProfile = async () => {
    if (!game || isSavingDisplayProfile) return;
    setIsSavingDisplayProfile(true);
    try {
      await displayProfileService.resetProfile(game.id);
      setDisplayProfile({
        gameId: game.id,
        enabled: false,
        displayId: null,
        deviceName: null,
        width: null,
        height: null,
        refreshRate: null,
        restoreOnExit: true,
        updatedAt: null,
        resolutionMode: "SYSTEM",
        refreshRateMode: "SYSTEM",
        hdrMode: "SYSTEM",
        rtxHdrPreset: null,
        rtxHdrPeakNits: 800,
      });
      setDisplayProfileMenu("root");
      setMessage("Perfil de pantalla restablecido a Auto.");
    } catch (error: unknown) {
      setMessage(displayProfileErrorMessage(error));
    } finally {
      setIsSavingDisplayProfile(false);
    }
  };

  const chooseFrameGeneration = async (multiplier: 0 | 2 | 3 | 4) => {
    if (!frameGenerationProfile || isSavingFrameGeneration) return;
    const next: FrameGenerationProfile = {
      ...frameGenerationProfile,
      enabled: multiplier !== 0,
      multiplier:
        multiplier === 0 ? frameGenerationProfile.multiplier : multiplier,
    };
    setIsSavingFrameGeneration(true);
    try {
      const saved = await frameGenerationService.saveProfile(next);
      setFrameGenerationProfile(saved);
      setDisplayProfileMenu("profile");
      setMessage(
        saved.restartRequired
          ? "Lossless Scaling debe reiniciarse antes del próximo lanzamiento."
          : saved.enabled && !saved.targetExecutable
            ? "Frame Generation preparado para el próximo lanzamiento; se detectará el ejecutable real."
            : `Frame Generation: ${frameGenerationLabel(saved)}.`,
      );
    } catch (error: unknown) {
      setMessage(frameGenerationErrorMessage(error));
    } finally {
      setIsSavingFrameGeneration(false);
    }
  };

  const openArtworkModifier = () => {
    if (!game || isRefreshingMetadata || isDownloadingMedia) return;
    engine.prepareScopeOpen(
      "artwork-modifier",
      engine.getActiveFocusId() ?? "details-back",
    );
    setMenuOpen(false);
    setArtworkModifierOpen(true);
  };

  const closeArtworkModifier = () => {
    engine.requestScopeRestore(
      "artwork-modifier",
      "details",
      `artwork-modifier-close-${game?.id ?? "unknown"}`,
    );
    engine.completePendingRestore("details", "details-back");
    setArtworkModifierOpen(false);
  };

  const downloadMedia = async () => {
    if (!game || isDownloadingMedia) return;
    setIsDownloadingMedia(true);
    const isEmulator = game.source === "emulator";
    setMessage(
      isEmulator
        ? "Descargando capturas desde LaunchBox…"
        : "Descargando multimedia desde Steam…",
    );
    try {
      const downloaded = isEmulator
        ? (await providerSettingsService.downloadLaunchBoxScreenshots(game.id))
            .length
        : await providerSettingsService.downloadSteamGameMedia(game.id);
      await queryClient.invalidateQueries({ queryKey: ["games"] });
      setMenuOpen(false);
      setMessage(
        isEmulator
          ? `Capturas de LaunchBox disponibles: ${downloaded}.`
          : `Multimedia descargado: ${downloaded} recursos.`,
      );
    } catch (error) {
      const detail =
        error instanceof Error ? error.message : "Error desconocido";
      setMessage(`No se pudo descargar el multimedia: ${detail}`);
    } finally {
      setIsDownloadingMedia(false);
    }
  };

  const refreshMetadata = async () => {
    if (!game || isRefreshingMetadata) return;
    setIsRefreshingMetadata(true);
    const isEmulator = game.source === "emulator";
    if (isEmulator) {
      setMessage("Actualizando metadatos desde el catálogo local…");
    }
    setMessage("Actualizando metadatos desde Steam…");
    if (isEmulator) {
      setMessage("Actualizando metadatos desde el catálogo local…");
    }
    try {
      const refreshResult = isEmulator
        ? await providerSettingsService.refreshGameMetadata(game.id)
        : null;
      if (!isEmulator) {
        await providerSettingsService.refreshSteamGameMetadata(game.id);
      }
      await queryClient.invalidateQueries({ queryKey: ["games"] });
      setMenuOpen(false);
      setMessage(
        refreshResult?.metadataResolved === false
          ? "Metadatos actualizados parcialmente; no se encontró una coincidencia segura en LaunchBox."
          : refreshResult?.status === "partial"
            ? "Metadatos actualizados; algunas capturas no están disponibles."
            : "Metadatos actualizados correctamente.",
      );
    } catch (error) {
      const detail = error instanceof Error ? error.message : "UNKNOWN_ERROR";
      const friendlyDetail = isEmulator
        ? (launchBoxErrorMessage(error) ??
          "No se pudo completar el refresco local.")
        : detail;
      setMessage(`No se pudieron actualizar los metadatos: ${friendlyDetail}`);
    } finally {
      setIsRefreshingMetadata(false);
    }
  };

  const toggleFavorite = async () => {
    if (!game || isUpdatingFavorite) return;
    const nextFavorite = !game.favorite;
    setIsUpdatingFavorite(true);
    try {
      await providerSettingsService.setGameFavorite(game.id, nextFavorite);
      await queryClient.invalidateQueries({ queryKey: ["games"] });
      setMessage(
        nextFavorite ? "Added to favorites." : "Removed from favorites.",
      );
    } catch (error) {
      const detail = error instanceof Error ? error.message : "Unknown error";
      setMessage(`Could not update favorite: ${detail}`);
    } finally {
      setIsUpdatingFavorite(false);
    }
  };

  const hideGame = async () => {
    if (!game || isUpdatingHidden) return;
    setIsUpdatingHidden(true);
    try {
      await providerSettingsService.setGameHidden(game.id, true);
      queryClient.setQueryData<Game[]>(["games"], (games) =>
        games?.map((candidate) =>
          candidate.id === game.id ? { ...candidate, hidden: true } : candidate,
        ),
      );
      setMenuOpen(false);
      handleClose();
    } catch (error) {
      const detail = error instanceof Error ? error.message : "Unknown error";
      setMessage(`Could not hide game: ${detail}`);
    } finally {
      setIsUpdatingHidden(false);
    }
  };

  const addRecommendedToWishlist = async (candidate: Game) => {
    if (candidate.favorite) {
      setMessage(`${candidate.title} ya está en tu lista de deseos.`);
      return;
    }
    try {
      await providerSettingsService.setGameFavorite(candidate.id, true);
      await queryClient.invalidateQueries({ queryKey: ["games"] });
      setMessage(`${candidate.title} se añadió a tu lista de deseos.`);
    } catch (error) {
      const detail = error instanceof Error ? error.message : "Unknown error";
      setMessage(`No se pudo añadir ${candidate.title} a tu lista: ${detail}`);
    }
  };

  const launchGame = () => {
    if (!game) return;
    useGameSessionStore
      .getState()
      .setReturnFocusId(engine.getActiveFocusId() ?? "details-play");
    void gameSessionService
      .start(game.id)
      .then((status) => useGameSessionStore.getState().applyStatus(status))
      .catch((error: unknown) => setMessage(gameSessionErrorMessage(error)));
  };

  const selectDetailsSection = (focusId: string) => {
    const nextSection =
      focusId === "details-tab-performance"
        ? "performance"
        : focusId === "details-tab-activity"
          ? "activity"
          : focusId === "details-tab-achievements"
            ? "achievements"
            : focusId === "details-tab-news"
              ? "news"
              : focusId === "details-tab-dlc"
                ? "dlc"
                : focusId === "details-tab-related"
                  ? "related"
                  : focusId === "details-tab-reviews"
                    ? "reviews"
                    : "summary";
    if (nextSection === activeSection) return;
    setDetailsContentDirection(
      DETAILS_TAB_ORDER.indexOf(nextSection) >
        DETAILS_TAB_ORDER.indexOf(activeSection)
        ? "forward"
        : "backward",
    );
    setActiveSection(nextSection);
  };

  const handleDetailsAction = (action: NavigationAction): boolean => {
    if (action !== "page-next" && action !== "page-previous") return false;

    const navigableTabs = DETAILS_TAB_ORDER.map(
      (section) => `details-tab-${section}`,
    );
    const currentIndex = DETAILS_TAB_ORDER.indexOf(activeSection);
    const offset = action === "page-next" ? 1 : -1;
    const nextTab = navigableTabs[currentIndex + offset];
    if (!nextTab) return true;

    selectDetailsSection(nextTab);
    engine.focus(nextTab);
    return true;
  };

  const refreshModes = useMemo(
    () =>
      displayModes
        .filter(
          (mode) =>
            mode.width === displayProfile?.width &&
            mode.height === displayProfile?.height,
        )
        .map((mode) => mode.refreshRate)
        .filter((rate, index, rates) => rates.indexOf(rate) === index)
        .sort((left, right) => left - right),
    [displayModes, displayProfile?.height, displayProfile?.width],
  );

  const steamDetails = game?.details?.steam;
  const screenshotUrls = useMemo(
    () =>
      game
        ? (game.screenshots.length > 0
            ? game.screenshots
            : (steamDetails?.screenshots ?? [])
          ).slice(0, 6)
        : [],
    [game, steamDetails?.screenshots],
  );

  if (!initialGame) return <p className="empty-state">Game not found.</p>;
  if (getDetailsReadiness(game, mediaQuery.status) === "waiting") return null;

  if (!game) return <p className="empty-state">Game not found.</p>;

  const playtimeMinutes =
    liveMetrics?.totalPlaytimeMinutes ??
    steamDetails?.totalPlaytimeMinutes ??
    game.playtimeMinutes;
  const lastPlayedAt =
    liveMetrics?.lastPlayedAt ??
    game.lastPlayedAt ??
    steamDetails?.lastPlayedAt ??
    null;
  const primaryGenre =
    game.genres[0] ?? steamDetails?.genres[0] ?? "Uncategorized";
  const achievementUnlocked =
    liveMetrics?.achievementUnlocked ??
    game.achievements?.unlocked ??
    steamDetails?.achievementUnlocked ??
    null;
  const achievementTotal =
    liveMetrics?.achievementTotal ??
    game.achievements?.total ??
    steamDetails?.achievementTotal ??
    null;
  const summaryDescription = toPlainText(
    steamDetails?.shortDescription ??
      steamDetails?.description ??
      game.description,
  );
  const launchboxDetails = game.details?.launchbox;
  const features = getFeatureList(game, steamDetails);
  const backgroundUrl = getGameBackgroundUrl(game);
  const activeDetailsTabFocusId = `details-tab-${activeSection}`;
  const openScreenshotViewer = (index: number) => {
    const focusId = `details-screenshot-${index}`;
    const sourceElement = engine.registry.get(focusId)?.element;
    const sourceRect = sourceElement?.getBoundingClientRect();
    const sourceStyle = sourceElement
      ? window.getComputedStyle(sourceElement)
      : null;
    setScreenshotViewerOrigin(
      sourceRect
        ? {
            left: sourceRect.left,
            top: sourceRect.top,
            width: sourceRect.width,
            height: sourceRect.height,
            borderRadius: sourceStyle?.borderRadius ?? "12px",
            boxShadow: sourceStyle?.boxShadow ?? "none",
          }
        : null,
    );
    engine.prepareScopeOpen("details-screenshot-viewer", focusId);
    setScreenshotViewerInitialIndex(index);
    setScreenshotViewerOpen(true);
  };
  const resolutionModes = displayModes.filter(
    (mode, index, modes) =>
      modes.findIndex(
        (candidate) =>
          candidate.width === mode.width && candidate.height === mode.height,
      ) === index,
  );

  return (
    <FocusScope
      scopeId="details"
      parentScopeId="product-shell"
      initialFocusId="details-play"
      restoreFocus
      rememberScroll
      trapFocus
      modal
      activateOnMount
      onAction={handleDetailsAction}
      onBack={() => {
        if (menuOpen) {
          if (
            displayProfileMenu === "resolution" ||
            displayProfileMenu === "refresh" ||
            displayProfileMenu === "hdr" ||
            displayProfileMenu === "frame-generation"
          ) {
            setDisplayProfileMenu("profile");
            return true;
          }
          if (displayProfileMenu === "profile") {
            setDisplayProfileMenu("root");
            return true;
          }
          setMenuOpen(false);
          return true;
        }
        handleClose();
        return true;
      }}
    >
      <section
        className="product-page details-view"
        aria-labelledby="details-heading"
      >
        <div
          className="details-hero"
          style={{ backgroundImage: `url("${backgroundUrl}")` }}
        >
          <SteamTrailer
            gameId={game.id}
            title={game.title}
            sourceUrls={steamDetails?.movies ?? []}
            posterUrl={backgroundUrl}
          />
          <div className="details-copy">
            <h1
              id="details-heading"
              className={game.logoUrl ? "visually-hidden" : undefined}
            >
              {game.title}
            </h1>
            {game.logoUrl && (
              <img
                src={game.logoUrl}
                alt={`${game.title} logo`}
                className="details-logo"
                draggable={false}
              />
            )}
            <div className="details-tags">
              <span>{formatProgress(game.progress)}</span>
              <span>{formatPlaytime(playtimeMinutes)}</span>
              <span>{primaryGenre}</span>
              <span>{game.provider}</span>
            </div>
            <div className="details-actions">
              <Focusable
                focusId="details-play"
                scopeId="details"
                className="primary-button"
                navigation={{
                  right: "details-favorite",
                  down: activeDetailsTabFocusId,
                }}
                onConfirm={launchGame}
              >
                <span className="details-button-input" aria-hidden="true">
                  A
                </span>
                {game.installed ? "Play" : "Add to library"}
              </Focusable>
              <Focusable
                focusId="details-favorite"
                scopeId="details"
                className="details-favorite-button"
                navigation={{
                  left: "details-play",
                  right: "details-back",
                  down: activeDetailsTabFocusId,
                }}
                ariaLabel={
                  game.favorite ? "Remove from favorites" : "Add to favorites"
                }
                ariaPressed={game.favorite}
                onConfirm={() => void toggleFavorite()}
              >
                <span aria-hidden="true">{game.favorite ? "♥" : "♡"}</span>
              </Focusable>
              <div className="details-menu-anchor">
                <Focusable
                  ref={menuButtonRef}
                  focusId="details-back"
                  scopeId="details"
                  className="details-menu-button"
                  navigation={{
                    left: "details-favorite",
                    down: activeDetailsTabFocusId,
                  }}
                  aria-label="More options"
                  ariaHaspopup="menu"
                  ariaExpanded={menuOpen}
                  onConfirm={toggleMenu}
                >
                  <span aria-hidden="true">...</span>
                </Focusable>
                {menuOpen && contextMenuPosition && (
                  <FocusScope
                    scopeId={CONTEXT_MENU_SCOPE_ID}
                    parentScopeId="details"
                    initialFocusId="details-display-profile"
                    restoreFocus
                    trapFocus
                    modal
                    activateOnMount
                    onBack={() => {
                      setDisplayProfileMenu("root");
                      setMenuOpen(false);
                      return true;
                    }}
                  >
                    {createPortal(
                      <div
                        className="details-context-menu"
                        style={{
                          top: contextMenuPosition.top,
                          left: contextMenuPosition.left,
                          transform: contextMenuPosition.transform,
                        }}
                        role="menu"
                        aria-label="Game options"
                      >
                        {displayProfileMenu === "root" && (
                          <>
                            <Focusable
                              focusId="details-display-profile"
                              scopeId={CONTEXT_MENU_SCOPE_ID}
                              className="details-context-menu-item"
                              role="menuitem"
                              navigation={{ down: "details-modify-artwork" }}
                              disabled={isSavingDisplayProfile}
                              onConfirm={openDisplayProfile}
                            >
                              <span>Display Profile</span>
                              <span className="details-context-menu-value">
                                {formatDisplayResolution(
                                  displayProfile?.resolutionMode === "CUSTOM"
                                    ? (displayProfile.width ?? null)
                                    : null,
                                  displayProfile?.resolutionMode === "CUSTOM"
                                    ? (displayProfile.height ?? null)
                                    : null,
                                )}{" "}
                                <span aria-hidden="true">›</span>
                              </span>
                            </Focusable>
                            <Focusable
                              focusId="details-modify-artwork"
                              scopeId={CONTEXT_MENU_SCOPE_ID}
                              className="details-context-menu-item"
                              role="menuitem"
                              disabled={
                                isRefreshingMetadata || isDownloadingMedia
                              }
                              onConfirm={openArtworkModifier}
                            >
                              Modificar arte
                            </Focusable>
                            <Focusable
                              focusId="details-update-metadata"
                              scopeId={CONTEXT_MENU_SCOPE_ID}
                              className="details-context-menu-item"
                              role="menuitem"
                              disabled={
                                isRefreshingMetadata || isDownloadingMedia
                              }
                              onConfirm={() => void refreshMetadata()}
                            >
                              {isRefreshingMetadata
                                ? "Actualizando metadatos…"
                                : "Actualizar Metadatos"}
                            </Focusable>
                            <Focusable
                              focusId="details-download-media"
                              scopeId={CONTEXT_MENU_SCOPE_ID}
                              className="details-context-menu-item"
                              role="menuitem"
                              disabled={
                                isRefreshingMetadata || isDownloadingMedia
                              }
                              navigation={{ down: "details-hide-game" }}
                              onConfirm={() => void downloadMedia()}
                            >
                              {isDownloadingMedia
                                ? "Descargando multimedia…"
                                : "Descargar multimedia"}
                            </Focusable>
                            <Focusable
                              focusId="details-hide-game"
                              scopeId={CONTEXT_MENU_SCOPE_ID}
                              className="details-context-menu-item"
                              role="menuitem"
                              disabled={isUpdatingHidden}
                              onConfirm={() => void hideGame()}
                            >
                              Ocultar juego
                            </Focusable>
                          </>
                        )}
                        {displayProfileMenu === "profile" && displayProfile && (
                          <>
                            <Focusable
                              focusId="details-display-mode"
                              scopeId={CONTEXT_MENU_SCOPE_ID}
                              className="details-context-menu-item"
                              role="menuitem"
                              navigation={{
                                down: "details-display-resolution",
                              }}
                              disabled={isSavingDisplayProfile}
                              onConfirm={() => void toggleDisplayProfileMode()}
                            >
                              <span>Mode</span>
                              <span className="details-context-menu-value">
                                {displayProfile.resolutionMode === "CUSTOM" ||
                                displayProfile.refreshRateMode === "CUSTOM"
                                  ? "Custom"
                                  : "System"}
                              </span>
                            </Focusable>
                            <Focusable
                              focusId="details-display-resolution"
                              scopeId={CONTEXT_MENU_SCOPE_ID}
                              className="details-context-menu-item"
                              role="menuitem"
                              navigation={{
                                up: "details-display-mode",
                                down: "details-display-refresh",
                              }}
                              disabled={
                                displayProfile.resolutionMode !== "CUSTOM" ||
                                isSavingDisplayProfile
                              }
                              onConfirm={() =>
                                setDisplayProfileMenu("resolution")
                              }
                            >
                              <span>Resolution</span>
                              <span className="details-context-menu-value">
                                {formatDisplayResolution(
                                  displayProfile.resolutionMode === "CUSTOM"
                                    ? displayProfile.width
                                    : null,
                                  displayProfile.resolutionMode === "CUSTOM"
                                    ? displayProfile.height
                                    : null,
                                )}{" "}
                                <span aria-hidden="true">›</span>
                              </span>
                            </Focusable>
                            <Focusable
                              focusId="details-display-refresh"
                              scopeId={CONTEXT_MENU_SCOPE_ID}
                              className="details-context-menu-item"
                              role="menuitem"
                              navigation={{
                                up: "details-display-resolution",
                                down: "details-frame-generation",
                              }}
                              disabled={
                                displayProfile.refreshRateMode !== "CUSTOM" ||
                                isSavingDisplayProfile
                              }
                              onConfirm={() => setDisplayProfileMenu("refresh")}
                            >
                              <span>Refresh Rate</span>
                              <span className="details-context-menu-value">
                                {formatDisplayRefreshRate(
                                  displayProfile.refreshRateMode === "CUSTOM"
                                    ? displayProfile.refreshRate
                                    : null,
                                )}{" "}
                                <span aria-hidden="true">›</span>
                              </span>
                            </Focusable>
                            <Focusable
                              focusId="details-frame-generation"
                              scopeId={CONTEXT_MENU_SCOPE_ID}
                              className="details-context-menu-item"
                              role="menuitem"
                              navigation={{
                                up: "details-display-refresh",
                                down: "details-display-hdr",
                              }}
                              disabled={isSavingFrameGeneration}
                              onConfirm={() =>
                                setDisplayProfileMenu("frame-generation")
                              }
                            >
                              <span>Frame Generation</span>
                              <span className="details-context-menu-value">
                                {frameGenerationLabel(frameGenerationProfile)}{" "}
                                <span aria-hidden="true">›</span>
                              </span>
                            </Focusable>
                            <Focusable
                              focusId="details-display-hdr"
                              scopeId={CONTEXT_MENU_SCOPE_ID}
                              className="details-context-menu-item"
                              role="menuitem"
                              navigation={{
                                up: "details-frame-generation",
                                down: "details-display-reset",
                              }}
                              disabled={isSavingDisplayProfile}
                              onConfirm={() => setDisplayProfileMenu("hdr")}
                            >
                              <span>HDR del juego</span>
                              <span className="details-context-menu-value">
                                {displayProfile.rtxHdrPreset
                                  ? `RTX HDR ${displayProfile.rtxHdrPreset === "NATURAL" ? "Natural" : "Vibrant"}`
                                  : displayProfile.hdrMode === "ON"
                                    ? "Native HDR"
                                    : displayProfile.hdrMode === "AUTO"
                                      ? "Automatic"
                                      : displayProfile.hdrMode === "OFF"
                                        ? "Disabled"
                                        : "System"}
                              </span>
                            </Focusable>
                            <Focusable
                              focusId="details-display-reset"
                              scopeId={CONTEXT_MENU_SCOPE_ID}
                              className="details-context-menu-item details-context-menu-item-danger"
                              role="menuitem"
                              navigation={{ up: "details-display-hdr" }}
                              disabled={isSavingDisplayProfile}
                              onConfirm={() => void resetDisplayProfile()}
                            >
                              Reset Profile
                            </Focusable>
                          </>
                        )}
                        {displayProfileMenu === "frame-generation" && (
                          <>
                            {([0, 2, 3, 4] as const).map(
                              (multiplier, index) => (
                                <Focusable
                                  key={multiplier}
                                  focusId={
                                    multiplier === 0
                                      ? "details-frame-generation-off"
                                      : `details-frame-generation-${multiplier}`
                                  }
                                  scopeId={CONTEXT_MENU_SCOPE_ID}
                                  className="details-context-menu-item"
                                  role="menuitem"
                                  navigation={{
                                    up:
                                      index === 0
                                        ? undefined
                                        : multiplier === 2
                                          ? "details-frame-generation-off"
                                          : `details-frame-generation-${multiplier - 1}`,
                                    down:
                                      index === 3
                                        ? undefined
                                        : multiplier === 0
                                          ? "details-frame-generation-2"
                                          : `details-frame-generation-${multiplier + 1}`,
                                  }}
                                  disabled={isSavingFrameGeneration}
                                  onConfirm={() =>
                                    void chooseFrameGeneration(multiplier)
                                  }
                                >
                                  {multiplier === 0
                                    ? "Off"
                                    : `LSFG ${multiplier}x${multiplier === 2 ? " (Recommended en NUC)" : ""}`}
                                </Focusable>
                              ),
                            )}
                          </>
                        )}
                        {displayProfileMenu === "hdr" && displayProfile && (
                          <>
                            {(
                              [
                                {
                                  id: "system",
                                  label: "System",
                                  mode: "SYSTEM",
                                  rtx: null,
                                },
                                {
                                  id: "automatic",
                                  label: "Automatic",
                                  mode: "AUTO",
                                  rtx: null,
                                },
                                {
                                  id: "native",
                                  label: "Native HDR",
                                  mode: "ON",
                                  rtx: null,
                                },
                                {
                                  id: "rtx-natural",
                                  label: "RTX HDR Natural",
                                  mode: null,
                                  rtx: "NATURAL",
                                },
                                {
                                  id: "rtx-vibrant",
                                  label: "RTX HDR Vibrant",
                                  mode: null,
                                  rtx: "VIBRANT",
                                },
                                {
                                  id: "disabled",
                                  label: "Disabled",
                                  mode: "OFF",
                                  rtx: null,
                                },
                              ] as const
                            )
                              .filter(
                                (choice) => !choice.rtx || rtxHdrAvailable,
                              )
                              .map((choice, index, values) => (
                                <Focusable
                                  key={choice.id}
                                  focusId={`details-display-hdr-${choice.id}`}
                                  scopeId={CONTEXT_MENU_SCOPE_ID}
                                  className="details-context-menu-item"
                                  role="menuitem"
                                  navigation={{
                                    up:
                                      index > 0
                                        ? `details-display-hdr-${values[index - 1].id}`
                                        : undefined,
                                    down:
                                      index < values.length - 1
                                        ? `details-display-hdr-${values[index + 1].id}`
                                        : undefined,
                                  }}
                                  onConfirm={() =>
                                    choice.rtx
                                      ? void chooseRtxHdr(choice.rtx)
                                      : void chooseDisplayHdr(
                                          choice.mode ?? "SYSTEM",
                                        )
                                  }
                                >
                                  {choice.label}
                                </Focusable>
                              ))}
                          </>
                        )}
                        {displayProfileMenu === "resolution" && (
                          <>
                            <Focusable
                              focusId="details-display-resolution-auto"
                              scopeId={CONTEXT_MENU_SCOPE_ID}
                              className="details-context-menu-item"
                              role="menuitem"
                              onConfirm={() => {
                                if (!displayProfile) return;
                                void saveProfile({
                                  ...displayProfile,
                                  enabled: false,
                                  resolutionMode: "SYSTEM",
                                });
                                setDisplayProfileMenu("profile");
                              }}
                            >
                              Auto / Desktop
                            </Focusable>
                            {resolutionModes.map((mode, index) => {
                              const focusId =
                                "details-display-resolution-" +
                                mode.width +
                                "x" +
                                mode.height;
                              const previous =
                                index === 0
                                  ? "details-display-resolution-auto"
                                  : "details-display-resolution-" +
                                    resolutionModes[index - 1].width +
                                    "x" +
                                    resolutionModes[index - 1].height;
                              const next =
                                index === resolutionModes.length - 1
                                  ? undefined
                                  : "details-display-resolution-" +
                                    resolutionModes[index + 1].width +
                                    "x" +
                                    resolutionModes[index + 1].height;
                              return (
                                <Focusable
                                  key={focusId}
                                  focusId={focusId}
                                  scopeId={CONTEXT_MENU_SCOPE_ID}
                                  className="details-context-menu-item"
                                  role="menuitem"
                                  navigation={{ up: previous, down: next }}
                                  onConfirm={() =>
                                    void chooseDisplayResolution(mode)
                                  }
                                >
                                  {mode.width} × {mode.height}
                                </Focusable>
                              );
                            })}
                          </>
                        )}
                        {displayProfileMenu === "refresh" &&
                          refreshModes.length === 0 && (
                            <p className="details-context-menu-empty">
                              No hay frecuencias disponibles.
                            </p>
                          )}
                        {displayProfileMenu === "refresh" &&
                          refreshModes.map((refreshRate, index) => (
                            <Focusable
                              key={refreshRate}
                              focusId={"details-display-refresh-" + refreshRate}
                              scopeId={CONTEXT_MENU_SCOPE_ID}
                              className="details-context-menu-item"
                              role="menuitem"
                              navigation={{
                                up:
                                  index > 0
                                    ? "details-display-refresh-" +
                                      refreshModes[index - 1]
                                    : undefined,
                                down:
                                  index < refreshModes.length - 1
                                    ? "details-display-refresh-" +
                                      refreshModes[index + 1]
                                    : undefined,
                              }}
                              onConfirm={() =>
                                void chooseDisplayRefreshRate(refreshRate)
                              }
                            >
                              {refreshRate} Hz
                            </Focusable>
                          ))}
                      </div>,
                      document.body,
                    )}
                  </FocusScope>
                )}
              </div>
            </div>
            {message && (
              <p className="details-message" aria-live="polite">
                {message}
              </p>
            )}
          </div>
          <div
            className={`details-metrics${
              isRefreshingMetrics ? " is-refreshing" : ""
            }`}
            aria-busy={isRefreshingMetrics}
            aria-label="Game statistics"
          >
            {isRefreshingMetrics && (
              <span className="details-metrics-refresh-status" role="status">
                Actualizando métricas…
              </span>
            )}
            <DetailsMetric
              icon="clock"
              label="Time played"
              value={formatPlaytime(playtimeMinutes)}
            />
            <DetailsMetric
              icon="calendar"
              label="Last played"
              value={formatLastPlayed(lastPlayedAt)}
            />
            <HltbDetailsMetric data={game.details?.hltb} />
            <DetailsMetric
              icon="trophy"
              label="Achievements"
              value={`${achievementUnlocked ?? "-"} / ${achievementTotal ?? "-"}`}
            />
            <DetailsMetric
              icon="players"
              label="Active players"
              value={
                liveMetrics?.activePlayers === null ||
                liveMetrics?.activePlayers === undefined
                  ? "-"
                  : formatCount(liveMetrics.activePlayers)
              }
            />
          </div>
        </div>
        <NavigationTabs
          groupId="details-sections"
          className={`details-tabs is-${activeSection}`}
          selectedId={`details-tab-${activeSection}`}
          onSelect={selectDetailsSection}
          activationMode="automatic"
          upTargetId="details-play"
          navigationRegion={{
            regionId: "details-sections",
            childRegionId: "details-content",
            entryFocusId:
              activeSection === "summary" && screenshotUrls.length > 0
                ? "details-screenshot-0"
                : activeSection === "performance"
                  ? "details-capability-native_hdr"
                  : undefined,
            entryFocusPolicy: "remembered",
          }}
          ariaLabel="Game sections"
        >
          <NavigationTab
            focusId="details-tab-summary"
            scopeId="details"
            className="details-tab"
          >
            Resumen
          </NavigationTab>
          <NavigationTab
            focusId="details-tab-performance"
            scopeId="details"
            className="details-tab"
          >
            Rendimiento
          </NavigationTab>
          <NavigationTab
            focusId="details-tab-activity"
            scopeId="details"
            className="details-tab"
          >
            Actividad
          </NavigationTab>
          <NavigationTab
            focusId="details-tab-achievements"
            scopeId="details"
            className="details-tab"
          >
            Logros
            <span className="details-tab-badge">
              {achievementUnlocked ?? "-"} / {achievementTotal ?? "-"}
            </span>
          </NavigationTab>
          <NavigationTab
            focusId="details-tab-news"
            scopeId="details"
            className="details-tab"
          >
            Noticias
          </NavigationTab>
          <NavigationTab
            focusId="details-tab-dlc"
            scopeId="details"
            className="details-tab"
          >
            DLC
          </NavigationTab>
          <NavigationTab
            focusId="details-tab-related"
            scopeId="details"
            className="details-tab"
          >
            Relacionados
          </NavigationTab>
          <NavigationTab
            focusId="details-tab-reviews"
            scopeId="details"
            className="details-tab"
          >
            Reseñas
          </NavigationTab>
        </NavigationTabs>
        <NavigationContent
          navigationRegion={{
            regionId: "details-content",
            parentRegionId: "details-sections",
          }}
        >
          <DetailsTabContent
            activeSection={activeSection}
            direction={detailsContentDirection}
          >
            {activeSection === "activity" ? (
              <ActivityView game={game} />
            ) : activeSection === "performance" ? (
              <GameCapabilitiesPanel
                gameId={game.id}
                steamAppId={steamDetails?.appId ?? null}
                screenshotUrls={screenshotUrls}
                backgroundUrl={backgroundUrl}
              />
            ) : activeSection === "achievements" ? (
              <AchievementsView gameId={game.id} />
            ) : activeSection === "news" ? (
              <NewsView game={game} />
            ) : activeSection === "dlc" ? (
              <DlcView game={game} />
            ) : activeSection === "related" ? (
              <RelatedGamesView
                game={game}
                games={getVisibleGames(games)}
                onMessage={setMessage}
                onAddToWishlist={(candidate) =>
                  void addRecommendedToWishlist(candidate)
                }
              />
            ) : activeSection === "reviews" ? (
              <ReviewsView game={game} />
            ) : (
              <section
                className="details-summary"
                aria-labelledby="details-summary-heading"
              >
                <div className="details-summary-copy">
                  <h2 id="details-summary-heading" className="visually-hidden">
                    Resumen
                  </h2>
                  <p className="details-summary-description">
                    {summaryDescription}
                  </p>
                  {launchboxDetails && (
                    <dl className="details-editorial-metadata">
                      {launchboxDetails.developer && (
                        <MetadataItem
                          label="Developer"
                          value={launchboxDetails.developer}
                        />
                      )}
                      {launchboxDetails.publisher && (
                        <MetadataItem
                          label="Publisher"
                          value={launchboxDetails.publisher}
                        />
                      )}
                      {launchboxDetails.releaseDate && (
                        <MetadataItem
                          label="Release date"
                          value={launchboxDetails.releaseDate}
                        />
                      )}
                      {launchboxDetails.communityRatingRaw !== null && (
                        <MetadataItem
                          label="LaunchBox Community"
                          value={formatLaunchBoxRating(launchboxDetails)}
                        />
                      )}
                      {launchboxDetails.localMultiplayer === "true" && (
                        <MetadataItem
                          label="Local Multiplayer"
                          value={
                            launchboxDetails.maxLocalPlayers
                              ? `Up to ${launchboxDetails.maxLocalPlayers} players`
                              : "Yes"
                          }
                        />
                      )}
                    </dl>
                  )}
                  <h3>Características</h3>
                  <ul className="details-feature-list">
                    {features.map((feature) => (
                      <li key={feature}>{feature}</li>
                    ))}
                  </ul>
                </div>
                <div className="details-summary-screenshots">
                  <p className="eyebrow">Capturas de pantalla</p>
                  {screenshotUrls.length > 0 ? (
                    <NavigationGrid
                      className="details-screenshot-grid"
                      groupId="details-summary-screenshots"
                      columns={3}
                      itemCount={screenshotUrls.length}
                    >
                      {screenshotUrls.map((screenshot, index) => (
                        <Focusable
                          key={`${game.id}-screenshot-${index}`}
                          focusId={`details-screenshot-${index}`}
                          scopeId="details"
                          gridIndex={index}
                          className="details-screenshot-focusable"
                          ariaLabel={`${game.title} screenshot ${index + 1}`}
                          onConfirm={() => openScreenshotViewer(index)}
                        >
                          <MediaImage
                            gameId={game.id}
                            mediaType="screenshot"
                            reactKey={`${game.id}-screenshot-${index}`}
                            src={screenshot}
                            alt=""
                            className="details-screenshot"
                            loading="eager"
                            decoding="async"
                            draggable={false}
                          />
                        </Focusable>
                      ))}
                    </NavigationGrid>
                  ) : shouldShowEmptyScreenshots(
                      game,
                      screenshotUrls.length,
                    ) ? (
                    <p className="details-empty-media">
                      No screenshots available.
                    </p>
                  ) : null}
                </div>
              </section>
            )}
          </DetailsTabContent>
        </NavigationContent>
        {screenshotViewerOpen && (
          <ScreenshotViewer
            gameTitle={game.title}
            gameId={game.id}
            screenshots={screenshotUrls}
            initialIndex={screenshotViewerInitialIndex}
            origin={screenshotViewerOrigin}
            onClose={() => setScreenshotViewerOpen(false)}
          />
        )}
      </section>
      {artworkModifierOpen && (
        <ArtworkModifierView game={game} onClose={closeArtworkModifier} />
      )}
    </FocusScope>
  );
}

type DetailsMetricIconName =
  "clock" | "calendar" | "chart" | "trophy" | "players";

function DetailsMetric({
  icon,
  label,
  value,
}: {
  icon: DetailsMetricIconName;
  label: string;
  value: string;
}) {
  return (
    <div className="details-metric">
      <DetailsMetricIcon name={icon} />
      <span className="details-metric-label">{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function HltbDetailsMetric({ data }: { data: HltbGameData | undefined }) {
  const hasData = data?.status === "matched" && data.mainStoryMinutes !== null;
  return (
    <div className="details-metric details-metric-hltb">
      <DetailsMetricIcon name="chart" />
      <span className="details-metric-label">HowLongToBeat</span>
      <strong>
        {hasData ? formatDuration(data.mainStoryMinutes) : "Sin datos"}
      </strong>
    </div>
  );
}

function MetadataItem({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

function formatLaunchBoxRating(
  details: NonNullable<GameDetails["launchbox"]>,
): string {
  const raw = details.communityRatingRaw;
  const scale = details.communityRatingScale;
  if (raw === null) return "Unavailable";
  const score = scale ? `${raw} / ${scale}` : `${raw}`;
  return details.communityRatingCount
    ? `${score} (${details.communityRatingCount} ratings)`
    : score;
}

function DetailsMetricIcon({ name }: { name: DetailsMetricIconName }) {
  const commonProps = {
    className: "details-metric-icon",
    fill: "none",
    stroke: "currentColor",
    strokeLinecap: "round" as const,
    strokeLinejoin: "round" as const,
    strokeWidth: 2,
    viewBox: "0 0 24 24",
    "aria-hidden": true,
  };

  return (
    <svg {...commonProps}>
      {name === "clock" && (
        <>
          <circle cx="12" cy="12" r="9" />
          <path d="M12 7v5l3 2" />
        </>
      )}
      {name === "calendar" && (
        <>
          <rect x="3" y="5" width="18" height="16" rx="2" />
          <path d="M16 3v4M8 3v4M3 10h18" />
          <path d="m9 15 2 2 4-4" />
        </>
      )}
      {name === "chart" && (
        <>
          <path d="M4 20V10M10 20V4M16 20v-7M3 20h18" />
          <path d="m5 8 5-4 6 5 4-5" />
        </>
      )}
      {name === "trophy" && (
        <>
          <path d="M8 4h8v4a4 4 0 0 1-8 0V4Z" />
          <path d="M8 6H5a3 3 0 0 0 3 3M16 6h3a3 3 0 0 1-3 3M12 12v4M8 20h8M10 16h4" />
        </>
      )}
      {name === "players" && (
        <>
          <circle cx="9" cy="8" r="3" />
          <path d="M3 19c0-3 2.5-5 6-5s6 2 6 5" />
          <path d="M16 6.5a2.5 2.5 0 0 1 0 5M18 14c2 .6 3 2 3 4" />
        </>
      )}
    </svg>
  );
}

function formatProgress(progress: number): string {
  return `${Math.round(progress)}%`;
}

function formatPlaytime(minutes: number): string {
  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;
  if (hours === 0) return `${remainingMinutes}m`;
  return remainingMinutes === 0
    ? `${hours}h`
    : `${hours}h ${remainingMinutes}m`;
}

const formatDuration = formatHltbDuration;

function formatCount(value: number): string {
  return new Intl.NumberFormat("en-US", { notation: "compact" }).format(value);
}

function formatLastPlayed(value: string | null): string {
  if (!value) return "Never";
  const date = parseSteamDate(value);
  if (Number.isNaN(date.getTime())) return value;
  const elapsedDays = Math.max(
    0,
    Math.floor((Date.now() - date.getTime()) / (1000 * 60 * 60 * 24)),
  );
  if (elapsedDays === 0) return "Today";
  return `${elapsedDays} days`;
}

function parseSteamDate(value: string): Date {
  const numericValue = Number(value);
  if (Number.isFinite(numericValue) && numericValue > 0) {
    return new Date(
      numericValue < 100_000_000_000 ? numericValue * 1000 : numericValue,
    );
  }
  return new Date(value);
}

function getFeatureList(
  game: Game,
  steamDetails: SteamGameDetails | undefined,
): string[] {
  const features = [
    steamDetails?.singlePlayer ? "Experiencia para un jugador" : null,
    steamDetails?.multiplayer ? "Multijugador" : null,
    steamDetails?.cloud ? "Guardado en la nube" : null,
    steamDetails?.tradingCards ? "Cartas coleccionables" : null,
    steamDetails?.workshop ? "Contenido del Steam Workshop" : null,
    ...(steamDetails?.categories ?? []).map(translateSteamFeature),
  ].filter((feature): feature is string => Boolean(feature));

  const uniqueFeatures = [...new Set(features)];
  if (uniqueFeatures.length > 0) return uniqueFeatures.slice(0, 5);
  return game.genres.slice(0, 4).map((genre) => `Género: ${genre}`);
}

function translateSteamFeature(value: string): string {
  const normalized = value.trim().toLowerCase();
  const translations: Record<string, string> = {
    "single-player": "Experiencia para un jugador",
    multiplayer: "Multijugador",
    "steam achievements": "Logros de Steam",
    "full controller support": "Compatibilidad con mando",
    "partial controller support": "Compatibilidad parcial con mando",
    "steam cloud": "Guardado en la nube",
    "steam trading cards": "Cartas coleccionables",
    "steam workshop": "Contenido del Steam Workshop",
  };
  return translations[normalized] ?? value;
}
