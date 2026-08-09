import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useState,
  type MutableRefObject,
} from "react";
import { useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import type { Game } from "../catalog/game-types";
import { Focusable } from "../../ui/navigation/focus/Focusable";
import {
  AvailabilityFeedback,
  AvailabilityFocusable,
} from "../../ui/navigation/actions/AvailabilityFocusable";
import type { ActionAvailability } from "../../ui/navigation/actions/availability-types";
import { NavigationGrid } from "../../ui/navigation/layouts/NavigationGrid";
import { NavigationDialog } from "../../ui/navigation/layouts/NavigationDialog";
import { GamepadTextInput } from "../../ui/keyboard/GamepadTextInput";
import { useNavigation } from "../../ui/navigation/navigation-context";
import { useFocusable } from "../../ui/navigation/focus/useFocusable";
import type { NavigationScreenDefinition } from "../../ui/navigation/screen/navigation-screen-contract";
import steamLogo from "../../assets/steam-logo.png";
import steamGridDbLogo from "../../assets/steamgriddb-logo.png";
import opencriticLogo from "../../assets/opencritic-logo.png";
import edenLogo from "../../assets/eden-logo.png";
import launchboxLogo from "../../assets/launchbox-logo.png";
import losslessScalingLogo from "../../assets/lossless-scaling-logo.webp";
import {
  createSettingsSaveCorrelationId,
  logSettingsDiagnostic,
  launchBoxErrorMessage,
  providerSettingsService,
  ProviderSettingsError,
} from "./provider-settings-service";
import type {
  SettingsLevel,
  SteamConfigurationState,
  SteamConfigurationStatus,
  SteamAchievementSyncStatus,
  SteamImageSyncStatus,
  SteamLibrarySyncScope,
  SteamProfile,
  SteamProfileState,
  SteamSyncStatus,
  HltbSettings,
  HltbCandidate,
  HltbPendingMatch,
  HltbSyncStatus,
  SteamGridDbConfigurationStatus,
  RapidApiReviewsConfigurationStatus,
  AIConfigurationStatus,
  AIConnectionStatus,
  StorageStatus,
  EdenExecutableInspection,
  EdenStatus,
} from "./settings-types";
import { validateSteamApiKey, validateSteamId64 } from "./settings-validation";
import {
  frameGenerationService,
  type LosslessScalingStatus,
} from "../launcher/frame-generation-service";
import type {
  LaunchBoxCatalogProgress,
  LaunchBoxCatalogStatus,
} from "./provider-settings-service";
import { launchBoxProgressPercent } from "./launchbox-progress";
import { NetworkView } from "./NetworkView";
import { DisplayView } from "./DisplayView";
import { BluetoothView } from "./BluetoothView";
import { BluetoothErrorBoundary } from "./BluetoothErrorBoundary";

export const SETTINGS_SCREEN_DEFINITION = {
  id: "settings",
  route: "settings",
  rootScope: { scopeId: "settings-shell" },
  initialFocus: "settings-integrations",
  regions: [
    {
      regionId: "settings-content",
      parentRegionId: "main-navigation",
      entryFocusId: "settings-integrations",
      exitFocusId: "main-nav-settings",
    },
  ],
  rowGroups: [{ groupId: "settings-items", orientation: "vertical" }],
  restorePolicy: { restoreFocus: true, rememberScroll: true },
} satisfies NavigationScreenDefinition;

const SETTINGS_ITEMS = [
  ["bluetooth", "Bluetooth", "Conecta mandos, audio y accesorios", "◉"],
  ["display", "Pantalla", "Monitor, resolución, frecuencia y escala", "▣"],
  ["network", "Red e Internet", "Estado, Ethernet y Wi-Fi", "⌁"],
  ["general", "General", "Idioma, inicio y notificaciones", "⚙"],
  ["appearance", "Apariencia", "Tema, fondo y efectos", "◐"],
  ["navigation", "Navegación", "Controles, atajos y comportamiento", "⌘"],
  ["library", "Biblioteca", "Filtros, vistas y contenido", "▦"],
  ["integrations", "Integraciones", "Conecta tus servicios y plataformas", "✚"],
  ["storage", "Almacenamiento", "Base de datos y espacio", "▤"],
  ["accessibility", "Accesibilidad", "Texto, contraste y más", "◉"],
  ["information", "Información", "Versión, licencia y créditos", "ⓘ"],
] as const;

const PROVIDERS = [
  ["eden", "Eden", "Nintendo Switch - discovery local de juegos", "E"],
  ["launchbox", "LaunchBox", "Metadatos y capturas para juegos emulados", "L"],
  ["steam", "Steam", "Sincroniza tu biblioteca y progreso", "◉"],
  ["hltb", "HowLongToBeat", "Duraciones estimadas para tus juegos", "H"],
  ["steamgriddb", "SteamGridDB", "Arte para personalizar tu biblioteca", "▦"],
  [
    "rapidapi-reviews",
    "OpenCritic / Metacritic",
    "Puntuaciones y reseñas de críticos",
    "R",
  ],
  [
    "ai-services",
    "Servicios IA",
    "Los Servicios IA permiten habilitar funciones avanzadas como consenso de reseñas, resúmenes de noticias, recomendaciones y futuras capacidades inteligentes de LumaDeck.",
    "✦",
  ],
  ["lossless-scaling", "Lossless Scaling", "Frame Generation por juego", "F"],
  ["epic", "Epic Games", "Sincroniza tu biblioteca", "E"],
  ["xbox", "Xbox", "Sincroniza logros y actividad", "X"],
  ["playstation", "PlayStation Network", "Sincroniza trofeos y juegos", "P"],
  ["ubisoft", "Ubisoft Connect", "Sincroniza tu biblioteca", "U"],
  ["gog", "GOG Galaxy", "Sincroniza tu biblioteca", "G"],
] as const;

function settingsAvailability(id: string): ActionAvailability {
  return id === "display" ||
    id === "network" ||
    id === "bluetooth" ||
    id === "integrations" ||
    id === "storage" ||
    id === "library"
    ? "available"
    : "coming-soon";
}

function providerAvailability(id: string): ActionAvailability {
  return id === "steam" ||
    id === "eden" ||
    id === "launchbox" ||
    id === "hltb" ||
    id === "steamgriddb" ||
    id === "rapidapi-reviews" ||
    id === "ai-services" ||
    id === "lossless-scaling"
    ? "available"
    : "coming-soon";
}

interface SettingsViewProps {
  games: readonly Game[];
  level: SettingsLevel;
  onLevelChange: (level: SettingsLevel) => void;
  onClose: () => void;
  backHandlerRef: MutableRefObject<(() => boolean) | null>;
}

export function SettingsView({
  games,
  level,
  onLevelChange,
  onClose,
  backHandlerRef,
}: SettingsViewProps) {
  const { engine } = useNavigation();
  const queryClient = useQueryClient();
  const [configuration, setConfiguration] =
    useState<SteamConfigurationStatus | null>(null);
  const [configurationState, setConfigurationState] =
    useState<SteamConfigurationState>("loading");
  const [profile, setProfile] = useState<SteamProfile | null>(null);
  const [profileState, setProfileState] = useState<SteamProfileState>("idle");
  const [profileError, setProfileError] = useState<string | null>(null);
  const [steamIdDraft, setSteamIdDraft] = useState("");
  const [apiKeyDraft, setApiKeyDraft] = useState("");
  const [editingSteamId, setEditingSteamId] = useState(false);
  const [editingApiKey, setEditingApiKey] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [confirmDisconnect, setConfirmDisconnect] = useState(false);
  const [syncStatus, setSyncStatus] = useState<SteamSyncStatus | null>(null);
  const [syncScope, setSyncScope] = useState<SteamLibrarySyncScope>("all");
  const [syncScopeSaving, setSyncScopeSaving] = useState(false);
  const [syncStarting, setSyncStarting] = useState(false);
  const [achievementSyncStatus, setAchievementSyncStatus] =
    useState<SteamAchievementSyncStatus | null>(null);
  const [achievementSyncStarting, setAchievementSyncStarting] = useState(false);
  const [imageSyncStatus, setImageSyncStatus] =
    useState<SteamImageSyncStatus | null>(null);
  const [imageSyncStarting, setImageSyncStarting] = useState(false);
  const [hltbSettings, setHltbSettings] = useState<HltbSettings | null>(null);
  const [hltbSyncStatus, setHltbSyncStatus] = useState<HltbSyncStatus | null>(
    null,
  );
  const [hltbSyncStarting, setHltbSyncStarting] = useState(false);
  const [hltbError, setHltbError] = useState<string | null>(null);
  const [steamGridDbConfiguration, setSteamGridDbConfiguration] =
    useState<SteamGridDbConfigurationStatus | null>(null);
  const [steamGridDbApiKeyDraft, setSteamGridDbApiKeyDraft] = useState("");
  const [steamGridDbEditing, setSteamGridDbEditing] = useState(false);
  const [steamGridDbSaving, setSteamGridDbSaving] = useState(false);
  const [steamGridDbDeleteConfirm, setSteamGridDbDeleteConfirm] =
    useState(false);
  const [steamGridDbError, setSteamGridDbError] = useState<string | null>(null);
  const [rapidApiReviewsConfiguration, setRapidApiReviewsConfiguration] =
    useState<RapidApiReviewsConfigurationStatus | null>(null);
  const [rapidApiReviewsApiKeyDraft, setRapidApiReviewsApiKeyDraft] =
    useState("");
  const [rapidApiReviewsEditing, setRapidApiReviewsEditing] = useState(false);
  const [rapidApiReviewsSaving, setRapidApiReviewsSaving] = useState(false);
  const [rapidApiReviewsDeleteConfirm, setRapidApiReviewsDeleteConfirm] =
    useState(false);
  const [rapidApiReviewsError, setRapidApiReviewsError] = useState<
    string | null
  >(null);
  const [aiConfiguration, setAiConfiguration] =
    useState<AIConfigurationStatus | null>(null);
  const [aiApiKeyDraft, setAiApiKeyDraft] = useState("");
  const [aiModelDraft, setAiModelDraft] = useState("google/gemini-2.5-flash");
  const [aiSaving, setAiSaving] = useState(false);
  const [aiTesting, setAiTesting] = useState(false);
  const [aiError, setAiError] = useState<string | null>(null);
  const [aiConnectionStatus, setAiConnectionStatus] =
    useState<AIConnectionStatus | null>(null);
  const [storageStatus, setStorageStatus] = useState<StorageStatus | null>(
    null,
  );
  const [storageLoading, setStorageLoading] = useState(false);
  const [storageConfirm, setStorageConfirm] = useState(false);
  const [storageDeleteSource, setStorageDeleteSource] = useState(true);
  const [storageError, setStorageError] = useState<string | null>(null);
  const [losslessScalingStatus, setLosslessScalingStatus] =
    useState<LosslessScalingStatus | null>(null);
  const [losslessScalingError, setLosslessScalingError] = useState<
    string | null
  >(null);
  const [edenStatus, setEdenStatus] = useState<EdenStatus | null>(null);
  const [edenInspection, setEdenInspection] =
    useState<EdenExecutableInspection | null>(null);
  const [edenExecutableDraft, setEdenExecutableDraft] = useState("");
  const [edenManualRoots, setEdenManualRoots] = useState<string[]>([]);
  const [edenLoading, setEdenLoading] = useState(false);
  const [edenError, setEdenError] = useState<string | null>(null);
  const [launchBoxStatus, setLaunchBoxStatus] =
    useState<LaunchBoxCatalogStatus | null>(null);
  const [launchBoxLoading, setLaunchBoxLoading] = useState(false);
  const [launchBoxUpdating, setLaunchBoxUpdating] = useState(false);
  const [launchBoxError, setLaunchBoxError] = useState<string | null>(null);
  const [launchBoxNotice, setLaunchBoxNotice] = useState<string | null>(null);
  const [availabilityFeedback, setAvailabilityFeedback] = useState<Exclude<
    ActionAvailability,
    "available" | "unavailable"
  > | null>(null);

  const loadProfile = useCallback(async () => {
    setProfile(null);
    setProfileState("loading");
    setProfileError(null);
    try {
      const steamProfile = await providerSettingsService.getSteamProfile();
      setProfile(steamProfile);
      setProfileState("loaded");
    } catch (error) {
      setProfileState(profileStateFromError(error));
      setProfileError(toSteamProfileErrorMessage(error));
    }
  }, []);

  const loadConfiguration = useCallback(async () => {
    setConfigurationState("loading");
    setErrorMessage(null);
    setProfile(null);
    setProfileState("loading");
    setProfileError(null);
    try {
      const status = await providerSettingsService.getSteamConfiguration();
      setConfiguration(status);
      setConfigurationState(status.status);
      if (status.status !== "configured") {
        setProfileState("idle");
        setSyncStatus(null);
        setAchievementSyncStatus(null);
        setImageSyncStatus(null);
        setSyncScope("all");
        return;
      }
      await loadProfile();
      const [steamSyncStatus, steamImageSyncStatus, steamSyncSettings] =
        await Promise.all([
          providerSettingsService.getSteamSyncStatus(),
          providerSettingsService.getSteamImageSyncStatus(),
          providerSettingsService.getSteamLibrarySyncSettings(),
        ]);
      setSyncStatus(steamSyncStatus);
      setImageSyncStatus(steamImageSyncStatus);
      setSyncScope(steamSyncSettings.scope);
    } catch (error) {
      setConfigurationState("save-error");
      setProfileState("error");
      setErrorMessage(toSafeErrorMessage(error));
    }
  }, [loadProfile]);

  useEffect(() => {
    if (level !== "steam" || configuration?.status !== "configured") return;
    let disposed = false;
    const refreshSyncStatus = async () => {
      try {
        const [status, imageStatus] = await Promise.all([
          providerSettingsService.getSteamSyncStatus(),
          providerSettingsService.getSteamImageSyncStatus(),
        ]);
        if (!disposed) {
          setSyncStatus(status);
          setImageSyncStatus(imageStatus);
        }
      } catch {
        // Keep Settings usable with a backend that predates the sync command.
      }
    };
    void refreshSyncStatus();
    const timer = window.setInterval(() => void refreshSyncStatus(), 500);
    return () => {
      disposed = true;
      window.clearInterval(timer);
    };
  }, [configuration?.status, level]);

  useEffect(() => {
    if (level !== "steam") return;
    void loadConfiguration();
  }, [level, loadConfiguration]);

  const loadStorageStatus = useCallback(async () => {
    setStorageLoading(true);
    setStorageError(null);
    try {
      setStorageStatus(await providerSettingsService.getStorageStatus());
    } catch (error) {
      setStorageError(toStorageErrorMessage(error));
    } finally {
      setStorageLoading(false);
    }
  }, []);

  const loadLaunchBoxStatus = useCallback(async () => {
    setLaunchBoxLoading(true);
    setLaunchBoxError(null);
    try {
      setLaunchBoxStatus(
        await providerSettingsService.getLaunchBoxCatalogStatus(),
      );
    } catch (error) {
      setLaunchBoxError(
        launchBoxErrorMessage(error) ?? toSafeErrorMessage(error),
      );
    } finally {
      setLaunchBoxLoading(false);
    }
  }, []);

  const updateLaunchBoxCatalog = async () => {
    if (launchBoxUpdating) return;
    setLaunchBoxUpdating(true);
    setLaunchBoxError(null);
    setLaunchBoxNotice(null);
    try {
      setLaunchBoxStatus(
        await providerSettingsService.refreshLaunchBoxCatalog(true),
      );
      setLaunchBoxNotice("Catálogo de LaunchBox actualizado correctamente.");
    } catch (error) {
      setLaunchBoxError(
        launchBoxErrorMessage(error) ?? toSafeErrorMessage(error),
      );
      try {
        setLaunchBoxStatus(
          await providerSettingsService.getLaunchBoxCatalogStatus(),
        );
      } catch {
        // Preserve the update error when status polling is unavailable.
      }
    } finally {
      setLaunchBoxUpdating(false);
    }
  };

  useEffect(() => {
    if (level !== "launchbox") return;
    setLaunchBoxNotice(null);
    void loadLaunchBoxStatus();
  }, [level, loadLaunchBoxStatus]);

  useEffect(() => {
    if (
      level !== "launchbox" ||
      (!launchBoxUpdating && launchBoxStatus?.status !== "updating")
    ) {
      return;
    }
    let disposed = false;
    const refreshLaunchBoxProgress = async () => {
      try {
        const status =
          await providerSettingsService.getLaunchBoxCatalogStatus();
        if (!disposed) setLaunchBoxStatus(status);
      } catch {
        // Keep the current real progress visible if a status read is transiently unavailable.
      }
    };
    void refreshLaunchBoxProgress();
    const timer = window.setInterval(
      () => void refreshLaunchBoxProgress(),
      1000,
    );
    return () => {
      disposed = true;
      window.clearInterval(timer);
    };
  }, [launchBoxStatus?.status, launchBoxUpdating, level]);

  useEffect(() => {
    if (level !== "storage") return;
    void loadStorageStatus();
  }, [level, loadStorageStatus]);

  useEffect(() => {
    if (level !== "storage") return;
    let disposed = false;
    const refreshStorageStatus = async () => {
      try {
        const status = await providerSettingsService.getStorageStatus();
        if (!disposed) setStorageStatus(status);
      } catch {
        // Keep the storage screen usable while a migration is finishing.
      }
    };
    const timer = window.setInterval(() => void refreshStorageStatus(), 500);
    return () => {
      disposed = true;
      window.clearInterval(timer);
    };
  }, [level]);

  const loadSteamGridDbConfiguration = useCallback(async () => {
    setSteamGridDbError(null);
    try {
      setSteamGridDbConfiguration(
        await providerSettingsService.getSteamGridDbConfiguration(),
      );
    } catch (error) {
      setSteamGridDbError(toSafeErrorMessage(error));
    }
  }, []);

  useEffect(() => {
    if (level !== "steamgriddb") return;
    setSteamGridDbApiKeyDraft("");
    setSteamGridDbEditing(false);
    setSteamGridDbDeleteConfirm(false);
    void loadSteamGridDbConfiguration();
  }, [level, loadSteamGridDbConfiguration]);

  const loadRapidApiReviewsConfiguration = useCallback(async () => {
    setRapidApiReviewsError(null);
    try {
      setRapidApiReviewsConfiguration(
        await providerSettingsService.getRapidApiReviewsConfiguration(),
      );
    } catch (error) {
      setRapidApiReviewsError(toSafeErrorMessage(error));
    }
  }, []);

  useEffect(() => {
    if (level !== "rapidapi-reviews") return;
    setRapidApiReviewsApiKeyDraft("");
    setRapidApiReviewsEditing(false);
    setRapidApiReviewsDeleteConfirm(false);
    void loadRapidApiReviewsConfiguration();
  }, [level, loadRapidApiReviewsConfiguration]);

  const loadAIConfiguration = useCallback(async () => {
    setAiError(null);
    try {
      const configuration = await providerSettingsService.getAIConfiguration();
      setAiConfiguration(configuration);
      setAiModelDraft(configuration.configuration.model);
      setAiConnectionStatus(configuration.connection);
    } catch (error) {
      setAiError(toSafeErrorMessage(error));
    }
  }, []);

  useEffect(() => {
    if (level !== "ai-services") return;
    setAiApiKeyDraft("");
    setAiConnectionStatus(null);
    void loadAIConfiguration();
  }, [level, loadAIConfiguration]);

  const loadHltb = useCallback(async () => {
    setHltbError(null);
    try {
      const [settings, status] = await Promise.all([
        providerSettingsService.getHltbSettings(),
        providerSettingsService.getHltbSyncStatus(),
      ]);
      setHltbSettings(settings);
      setHltbSyncStatus(status);
    } catch (error) {
      setHltbError(toHltbErrorMessage(error));
    }
  }, []);

  useEffect(() => {
    if (level !== "hltb") return;
    void loadHltb();
    let disposed = false;
    const timer = window.setInterval(() => {
      void providerSettingsService
        .getHltbSyncStatus()
        .then((status) => {
          if (!disposed) setHltbSyncStatus(status);
        })
        .catch(() => undefined);
    }, 500);
    return () => {
      disposed = true;
      window.clearInterval(timer);
    };
  }, [level, loadHltb]);

  useEffect(() => {
    if (level !== "lossless-scaling") return;
    setLosslessScalingError(null);
    void frameGenerationService
      .getStatus()
      .then(setLosslessScalingStatus)
      .catch(() =>
        setLosslessScalingError("No se pudo consultar Lossless Scaling."),
      );
  }, [level]);

  useEffect(() => {
    setAvailabilityFeedback(null);
  }, [level]);

  const loadEdenStatus = useCallback(async () => {
    setEdenLoading(true);
    setEdenError(null);
    try {
      const status = await providerSettingsService.getEdenStatus();
      setEdenStatus(status);
      setEdenExecutableDraft(status.executablePath ?? "");
    } catch (error) {
      setEdenError(toSafeErrorMessage(error));
    } finally {
      setEdenLoading(false);
    }
  }, []);

  useEffect(() => {
    if (level !== "eden") return;
    setEdenInspection(null);
    setEdenManualRoots([]);
    void loadEdenStatus();
  }, [level, loadEdenStatus]);

  const selectEdenExecutable = async () => {
    setEdenError(null);
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "Eden executable", extensions: ["exe"] }],
      });
      if (typeof selected !== "string") return;
      setEdenExecutableDraft(selected);
      setEdenInspection(
        await providerSettingsService.inspectEdenExecutable(selected),
      );
    } catch (error) {
      setEdenError(toSafeErrorMessage(error));
    }
  };

  const selectEdenGameFolder = async () => {
    setEdenError(null);
    try {
      const selected = await open({ directory: true, multiple: false });
      if (typeof selected === "string") {
        setEdenManualRoots((roots) =>
          roots.includes(selected) ? roots : [...roots, selected],
        );
      }
    } catch (error) {
      setEdenError(toSafeErrorMessage(error));
    }
  };

  const connectEden = async () => {
    if (!edenExecutableDraft) return;
    setEdenLoading(true);
    setEdenError(null);
    try {
      setEdenStatus(
        await providerSettingsService.connectEden(
          edenExecutableDraft,
          edenManualRoots,
        ),
      );
      await queryClient.invalidateQueries({ queryKey: ["games"] });
    } catch (error) {
      setEdenError(toSafeErrorMessage(error));
    } finally {
      setEdenLoading(false);
    }
  };

  const rescanEden = async () => {
    setEdenLoading(true);
    setEdenError(null);
    try {
      setEdenStatus(await providerSettingsService.rescanEden());
      await queryClient.invalidateQueries({ queryKey: ["games"] });
    } catch (error) {
      setEdenError(toSafeErrorMessage(error));
    } finally {
      setEdenLoading(false);
    }
  };

  const disconnectEden = async () => {
    setEdenLoading(true);
    setEdenError(null);
    try {
      setEdenStatus(await providerSettingsService.disconnectEden());
      setEdenInspection(null);
      setEdenExecutableDraft("");
      await queryClient.invalidateQueries({ queryKey: ["games"] });
    } catch (error) {
      setEdenError(toSafeErrorMessage(error));
    } finally {
      setEdenLoading(false);
    }
  };

  const focusTarget = useMemo(() => {
    if (level === "settings") return "settings-integrations";
    if (level === "display") return "display-resolution";
    if (level === "network") return "network-refresh";
    if (level === "bluetooth") return "bluetooth-discovery";
    if (level === "library") return "settings-library";
    if (level === "integrations") return "integration-steam";
    if (level === "hltb") return "hltb-sync-missing";
    if (level === "steamgriddb") {
      if (steamGridDbDeleteConfirm) return "steamgriddb-delete-cancel";
      if (steamGridDbEditing || !steamGridDbConfiguration?.apiKeyConfigured) {
        return "steamgriddb-api-key-input";
      }
      return "steamgriddb-change-key";
    }
    if (level === "rapidapi-reviews") {
      if (rapidApiReviewsDeleteConfirm) return "rapidapi-reviews-delete-cancel";
      if (
        rapidApiReviewsEditing ||
        !rapidApiReviewsConfiguration?.apiKeyConfigured
      ) {
        return "rapidapi-reviews-api-key-input";
      }
      return "rapidapi-reviews-change-key";
    }
    if (level === "ai-services") return "ai-model-input";
    if (level === "lossless-scaling") return "lossless-scaling-open";
    if (level === "launchbox") {
      return "launchbox-catalog-action";
    }
    if (level === "eden") {
      if (edenStatus?.status === "ready") return "eden-rescan";
      return edenInspection ? "eden-connect" : "eden-select-executable";
    }
    if (level === "storage") return "storage-migrate";
    if (editingSteamId) return "steam-id-input";
    if (editingApiKey) return "steam-api-key-input";
    if (configuration?.status === "configured") return "steam-change-id";
    return "steam-id-input";
  }, [
    configuration?.status,
    editingApiKey,
    editingSteamId,
    level,
    steamGridDbConfiguration?.apiKeyConfigured,
    steamGridDbDeleteConfirm,
    steamGridDbEditing,
    rapidApiReviewsConfiguration?.apiKeyConfigured,
    rapidApiReviewsDeleteConfirm,
    rapidApiReviewsEditing,
    edenInspection,
    edenStatus?.status,
  ]);

  useLayoutEffect(() => {
    if (level === "settings") return;
    if (level === "steam" && configurationState === "loading") return;
    if (level === "storage" && storageLoading) return;
    if (engine.getActiveScopeId() !== "settings-shell") return;
    if (engine.focus(focusTarget)) return;
    let frame: number | null = window.requestAnimationFrame(() => {
      frame = null;
      engine.focus(focusTarget);
    });
    return () => {
      if (frame !== null) window.cancelAnimationFrame(frame);
    };
  }, [
    configurationState,
    editingApiKey,
    editingSteamId,
    engine,
    focusTarget,
    level,
    storageLoading,
  ]);

  const handleBack = useCallback(() => {
    if (confirmDisconnect) {
      setConfirmDisconnect(false);
      return true;
    }
    if (storageConfirm) {
      setStorageConfirm(false);
      return true;
    }
    if (steamGridDbDeleteConfirm) {
      setSteamGridDbDeleteConfirm(false);
      return true;
    }
    if (steamGridDbEditing) {
      setSteamGridDbEditing(false);
      setSteamGridDbApiKeyDraft("");
      return true;
    }
    if (rapidApiReviewsDeleteConfirm) {
      setRapidApiReviewsDeleteConfirm(false);
      return true;
    }
    if (rapidApiReviewsEditing) {
      setRapidApiReviewsEditing(false);
      setRapidApiReviewsApiKeyDraft("");
      return true;
    }
    if (level === "ai-services") {
      onLevelChange("integrations");
      return true;
    }
    if (level === "network") {
      onLevelChange("settings");
      return true;
    }
    if (level === "bluetooth") {
      onLevelChange("settings");
      return true;
    }
    if (level === "display") {
      onLevelChange("settings");
      return true;
    }
    if (editingSteamId || editingApiKey) {
      setEditingSteamId(false);
      setEditingApiKey(false);
      setSteamIdDraft("");
      setApiKeyDraft("");
      return true;
    }
    if (level === "steam") {
      onLevelChange("integrations");
      return true;
    }
    if (level === "hltb") {
      onLevelChange("integrations");
      return true;
    }
    if (level === "steamgriddb") {
      onLevelChange("integrations");
      return true;
    }
    if (level === "rapidapi-reviews") {
      onLevelChange("integrations");
      return true;
    }
    if (level === "lossless-scaling") {
      onLevelChange("integrations");
      return true;
    }
    if (level === "launchbox") {
      onLevelChange("integrations");
      return true;
    }
    if (level === "eden") {
      onLevelChange("integrations");
      return true;
    }
    if (level === "integrations") {
      onLevelChange("settings");
      return true;
    }
    if (level === "storage") {
      onLevelChange("settings");
      return true;
    }
    if (level === "library") {
      onLevelChange("settings");
      return true;
    }
    onClose();
    return true;
  }, [
    confirmDisconnect,
    editingApiKey,
    editingSteamId,
    level,
    onClose,
    onLevelChange,
    storageConfirm,
    steamGridDbDeleteConfirm,
    steamGridDbEditing,
    rapidApiReviewsDeleteConfirm,
    rapidApiReviewsEditing,
  ]);

  useLayoutEffect(() => {
    backHandlerRef.current = handleBack;
    return () => {
      if (backHandlerRef.current === handleBack) backHandlerRef.current = null;
    };
  }, [backHandlerRef, handleBack]);

  const openSteamIdEditor = () => {
    setErrorMessage(null);
    setSteamIdDraft("");
    setEditingSteamId(true);
  };

  const openApiKeyEditor = () => {
    setErrorMessage(null);
    setApiKeyDraft("");
    setEditingApiKey(true);
  };

  const openDisconnectDialog = () => {
    engine.prepareScopeOpen("settings-confirm-dialog", "steam-disconnect");
    setConfirmDisconnect(true);
  };

  const openSteamGridDbApiKeyEditor = () => {
    setSteamGridDbError(null);
    setSteamGridDbApiKeyDraft("");
    setSteamGridDbEditing(true);
  };

  const saveSteamGridDbApiKey = async () => {
    if (steamGridDbSaving) return;
    setSteamGridDbSaving(true);
    setSteamGridDbError(null);
    try {
      setSteamGridDbConfiguration(
        await providerSettingsService.saveSteamGridDbApiKey(
          steamGridDbApiKeyDraft,
        ),
      );
      setSteamGridDbApiKeyDraft("");
      setSteamGridDbEditing(false);
    } catch (error) {
      setSteamGridDbError(toSafeErrorMessage(error));
    } finally {
      setSteamGridDbSaving(false);
    }
  };

  const openSteamGridDbDeleteDialog = () => {
    engine.prepareScopeOpen(
      "steamgriddb-confirm-dialog",
      "steamgriddb-delete-cancel",
    );
    setSteamGridDbDeleteConfirm(true);
  };

  const deleteSteamGridDbApiKey = async () => {
    setSteamGridDbError(null);
    try {
      setSteamGridDbConfiguration(
        await providerSettingsService.deleteSteamGridDbApiKey(),
      );
      setSteamGridDbDeleteConfirm(false);
    } catch (error) {
      setSteamGridDbError(toSafeErrorMessage(error));
    }
  };

  const openRapidApiReviewsApiKeyEditor = () => {
    setRapidApiReviewsError(null);
    setRapidApiReviewsApiKeyDraft("");
    setRapidApiReviewsEditing(true);
  };

  const saveRapidApiReviewsApiKey = async () => {
    if (rapidApiReviewsSaving) return;
    setRapidApiReviewsSaving(true);
    setRapidApiReviewsError(null);
    try {
      setRapidApiReviewsConfiguration(
        await providerSettingsService.saveRapidApiReviewsApiKey(
          rapidApiReviewsApiKeyDraft,
        ),
      );
      setRapidApiReviewsApiKeyDraft("");
      setRapidApiReviewsEditing(false);
    } catch (error) {
      setRapidApiReviewsError(toSafeErrorMessage(error));
    } finally {
      setRapidApiReviewsSaving(false);
    }
  };

  const openRapidApiReviewsDeleteDialog = () => {
    engine.prepareScopeOpen(
      "rapidapi-reviews-confirm-dialog",
      "rapidapi-reviews-delete-cancel",
    );
    setRapidApiReviewsDeleteConfirm(true);
  };

  const deleteRapidApiReviewsApiKey = async () => {
    setRapidApiReviewsError(null);
    try {
      setRapidApiReviewsConfiguration(
        await providerSettingsService.deleteRapidApiReviewsApiKey(),
      );
      setRapidApiReviewsDeleteConfirm(false);
    } catch (error) {
      setRapidApiReviewsError(toSafeErrorMessage(error));
    }
  };

  const saveAIConfiguration = async () => {
    if (aiSaving) return;
    setAiSaving(true);
    setAiError(null);
    try {
      const configuration = await providerSettingsService.saveAIConfiguration(
        "openrouter",
        aiModelDraft,
        aiApiKeyDraft,
      );
      setAiConfiguration(configuration);
      setAiModelDraft(configuration.configuration.model);
      setAiApiKeyDraft("");
      setAiConnectionStatus(configuration.connection);
    } catch (error) {
      setAiError(toSafeErrorMessage(error));
    } finally {
      setAiSaving(false);
    }
  };

  const testAIConnection = async () => {
    if (aiTesting) return;
    setAiTesting(true);
    setAiError(null);
    setAiConnectionStatus({ state: "connecting" });
    try {
      const status = await providerSettingsService.testAIConnection(
        "openrouter",
        aiModelDraft,
        aiApiKeyDraft,
      );
      setAiConnectionStatus(status);
    } catch (error) {
      setAiConnectionStatus({ state: "error" });
      setAiError(toSafeErrorMessage(error));
    } finally {
      setAiTesting(false);
    }
  };

  const openStorageMigrationDialog = () => {
    if (!storageStatus || storageStatus.migration.status === "running") return;
    engine.prepareScopeOpen(
      "storage-confirm-dialog",
      "storage-migration-cancel",
    );
    setStorageConfirm(true);
  };

  const startStorageMigration = async () => {
    if (!storageStatus || storageStatus.migration.status === "running") return;
    const targetMode =
      storageStatus.mode === "portable" ? "appData" : "portable";
    setStorageConfirm(false);
    setStorageError(null);
    try {
      await providerSettingsService.migrateStorage(
        targetMode,
        storageDeleteSource,
      );
      await loadStorageStatus();
    } catch (error) {
      setStorageError(toStorageErrorMessage(error));
      try {
        setStorageStatus(await providerSettingsService.getStorageStatus());
      } catch {
        // Keep the migration error visible if status refresh is unavailable.
      }
    }
  };

  const saveSteamId = async () => {
    const correlationId = createSettingsSaveCorrelationId();
    logSettingsDiagnostic("SAVE_CLICKED", {
      correlationId,
      command: configuration?.accountId
        ? "update_steam_id"
        : "save_steam_account_configuration",
      payload: {
        steamId64Length: steamIdDraft.trim().length,
        apiKeyPresent: apiKeyDraft.trim().length > 0,
      },
      previousUiState: {
        configurationState,
        configurationStatus: configuration?.status ?? "not-configured",
        accountIdPresent: Boolean(configuration?.accountId),
      },
    });
    const validation = validateSteamId64(steamIdDraft);
    if (validation.error || !validation.value) {
      logSettingsDiagnostic("VALIDATION_REJECTED", {
        correlationId,
        errorCode: "INVALID_STEAM_ID",
      });
      setErrorMessage(validation.error ?? "SteamID64 inválido.");
      return;
    }
    setConfigurationState("saving");
    setErrorMessage(null);
    try {
      const status = configuration?.accountId
        ? await providerSettingsService.updateSteamId(
            validation.value,
            correlationId,
          )
        : await providerSettingsService.saveSteamConfiguration(
            validation.value,
            apiKeyDraft,
            correlationId,
          );
      setConfiguration(status);
      setConfigurationState(status.status);
      if (status.status === "configured") await loadProfile();
      else {
        setProfile(null);
        setProfileState("idle");
      }
      logSettingsDiagnostic("UI_STATE_AFTER", {
        correlationId,
        state: status.status,
        accountIdPresent: Boolean(status.accountId),
      });
      setEditingSteamId(false);
      if (!configuration?.accountId) {
        setApiKeyDraft("");
        setEditingApiKey(false);
      }
    } catch (error) {
      setConfigurationState("save-error");
      setErrorMessage(toSafeErrorMessage(error));
      logSettingsDiagnostic("UI_STATE_AFTER", {
        correlationId,
        state: "save-error",
        errorCode:
          error instanceof ProviderSettingsError ? error.code : "UNKNOWN",
      });
    }
  };

  const saveApiKey = async () => {
    const correlationId = createSettingsSaveCorrelationId();
    logSettingsDiagnostic("SAVE_CLICKED", {
      correlationId,
      command: configuration?.accountId
        ? "replace_steam_api_key"
        : "save_steam_account_configuration",
      payload: {
        steamId64Length: steamIdDraft.trim().length,
        apiKeyPresent: apiKeyDraft.trim().length > 0,
      },
      previousUiState: {
        configurationState,
        configurationStatus: configuration?.status ?? "not-configured",
        accountIdPresent: Boolean(configuration?.accountId),
      },
    });
    const validation = validateSteamApiKey(apiKeyDraft);
    if (validation.error || !validation.value) {
      logSettingsDiagnostic("VALIDATION_REJECTED", {
        correlationId,
        errorCode: "INVALID_API_KEY",
      });
      setErrorMessage(validation.error ?? "API Key inválida.");
      return;
    }
    setConfigurationState("saving");
    setErrorMessage(null);
    try {
      const status = configuration?.accountId
        ? await providerSettingsService.replaceSteamApiKey(
            validation.value,
            correlationId,
          )
        : await providerSettingsService.saveSteamConfiguration(
            steamIdDraft,
            validation.value,
            correlationId,
          );
      setConfiguration(status);
      setConfigurationState(status.status);
      if (status.status === "configured") await loadProfile();
      else {
        setProfile(null);
        setProfileState("idle");
      }
      logSettingsDiagnostic("UI_STATE_AFTER", {
        correlationId,
        state: status.status,
        accountIdPresent: Boolean(status.accountId),
      });
      setEditingApiKey(false);
      setApiKeyDraft("");
    } catch (error) {
      setConfigurationState("save-error");
      setErrorMessage(toSafeErrorMessage(error));
      logSettingsDiagnostic("UI_STATE_AFTER", {
        correlationId,
        state: "save-error",
        errorCode:
          error instanceof ProviderSettingsError ? error.code : "UNKNOWN",
      });
    }
  };

  const disconnect = async () => {
    if (!configuration?.accountId) return;
    setConfigurationState("saving");
    setErrorMessage(null);
    try {
      const status = await providerSettingsService.disconnect(
        configuration.accountId,
      );
      setConfiguration(status);
      setConfigurationState(status.status);
      setProfile(null);
      setProfileState("idle");
      setProfileError(null);
      setConfirmDisconnect(false);
      setEditingSteamId(false);
      setEditingApiKey(false);
    } catch (error) {
      setConfigurationState("save-error");
      setErrorMessage(toSafeErrorMessage(error));
    }
  };

  const startSteamSync = async () => {
    if (
      syncStarting ||
      syncStatus?.status === "running" ||
      imageSyncStarting ||
      imageSyncStatus?.status === "running"
    )
      return;
    setSyncStarting(true);
    setErrorMessage(null);
    setSyncStatus((current) => ({
      status: "running",
      foundCount: current?.foundCount ?? 0,
      createdCount: 0,
      updatedCount: 0,
      progressCompleted: 0,
      progressTotal: current?.progressTotal ?? 0,
    }));
    try {
      const result = await providerSettingsService.syncSteamLibrary();
      void queryClient.invalidateQueries({ queryKey: ["games"] });
      setSyncStatus((current) => ({
        status: result.status,
        foundCount: result.foundCount,
        createdCount: result.createdCount,
        updatedCount: result.updatedCount,
        progressCompleted: result.foundCount,
        progressTotal: result.foundCount,
        durationMs: result.durationMs,
        completedAt: result.completedAt,
        startedAt: current?.startedAt,
      }));
      if (!hltbSettings) {
        void providerSettingsService
          .getHltbSettings()
          .then((settings) => {
            setHltbSettings(settings);
            if (settings.enabled && settings.syncWithSteam) {
              return providerSettingsService.syncHltbLibrary(true);
            }
            return null;
          })
          .then((status) => {
            if (status) setHltbSyncStatus(status);
          })
          .catch(() => undefined);
      } else if (hltbSettings.enabled && hltbSettings.syncWithSteam) {
        void providerSettingsService
          .syncHltbLibrary(true)
          .then((status) => setHltbSyncStatus(status))
          .catch(() => undefined);
      }
    } catch (error) {
      if (
        !(error instanceof ProviderSettingsError) ||
        error.code !== "STEAM_SYNC_CANCELLED"
      ) {
        setErrorMessage(toSyncErrorMessage(error));
      }
    } finally {
      setSyncStarting(false);
      try {
        setSyncStatus(await providerSettingsService.getSteamSyncStatus());
      } catch {
        // Keep the command result visible when status polling is unavailable.
      }
    }
  };

  const cancelSteamSync = async () => {
    if (syncStatus?.status !== "running") return;
    try {
      setSyncStatus(await providerSettingsService.cancelSteamLibrarySync());
    } catch (error) {
      setErrorMessage(toSyncErrorMessage(error));
    }
  };

  const startSteamAchievementSync = async () => {
    if (
      achievementSyncStarting ||
      syncStarting ||
      syncStatus?.status === "running" ||
      imageSyncStarting ||
      imageSyncStatus?.status === "running"
    )
      return;
    setAchievementSyncStarting(true);
    setErrorMessage(null);
    setAchievementSyncStatus({
      status: "running",
      foundCount: 0,
      updatedCount: 0,
      skippedCount: 0,
    });
    try {
      const result = await providerSettingsService.syncSteamAchievements();
      void queryClient.invalidateQueries({ queryKey: ["games"] });
      setAchievementSyncStatus({
        status: result.status,
        foundCount: result.foundCount,
        updatedCount: result.updatedCount,
        skippedCount: result.skippedCount,
        durationMs: result.durationMs,
        completedAt: result.completedAt,
      });
    } catch (error) {
      setAchievementSyncStatus((current) => ({
        status: "error",
        foundCount: current?.foundCount ?? 0,
        updatedCount: current?.updatedCount ?? 0,
        skippedCount: current?.skippedCount ?? 0,
      }));
      setErrorMessage(toAchievementSyncErrorMessage(error));
    } finally {
      setAchievementSyncStarting(false);
    }
  };

  const saveSteamSyncScope = async (scope: SteamLibrarySyncScope) => {
    if (syncScopeSaving || syncScope === scope) return;
    setSyncScopeSaving(true);
    setErrorMessage(null);
    try {
      const settings =
        await providerSettingsService.setSteamLibrarySyncScope(scope);
      setSyncScope(settings.scope);
    } catch (error) {
      setErrorMessage(toSyncErrorMessage(error));
    } finally {
      setSyncScopeSaving(false);
    }
  };

  const startSteamImageSync = async () => {
    if (
      imageSyncStarting ||
      imageSyncStatus?.status === "running" ||
      syncStarting ||
      syncStatus?.status === "running"
    )
      return;
    setImageSyncStarting(true);
    setErrorMessage(null);
    setImageSyncStatus((current) => ({
      status: "running",
      foundCount: current?.foundCount ?? 0,
      downloadedCount: 0,
      skippedCount: 0,
      progressCompleted: 0,
      progressTotal: current?.progressTotal ?? 0,
    }));
    try {
      const result = await providerSettingsService.syncSteamImages();
      void queryClient.invalidateQueries({ queryKey: ["games"] });
      setImageSyncStatus((current) => ({
        status: result.status,
        foundCount: result.foundCount,
        downloadedCount: result.downloadedCount,
        skippedCount: result.skippedCount,
        progressCompleted: result.foundCount,
        progressTotal: result.foundCount,
        durationMs: result.durationMs,
        completedAt: result.completedAt,
        startedAt: current?.startedAt,
      }));
    } catch (error) {
      if (
        !(error instanceof ProviderSettingsError) ||
        error.code !== "STEAM_IMAGE_SYNC_CANCELLED"
      ) {
        setErrorMessage(toImageSyncErrorMessage(error));
      }
    } finally {
      setImageSyncStarting(false);
      try {
        setImageSyncStatus(
          await providerSettingsService.getSteamImageSyncStatus(),
        );
      } catch {
        // Keep the command result visible when status polling is unavailable.
      }
    }
  };

  const cancelSteamImageSync = async () => {
    if (imageSyncStatus?.status !== "running") return;
    try {
      setImageSyncStatus(await providerSettingsService.cancelSteamImageSync());
    } catch (error) {
      setErrorMessage(toImageSyncErrorMessage(error));
    }
  };

  const updateHltbSetting = async (key: keyof HltbSettings, value: boolean) => {
    if (!hltbSettings) return;
    try {
      setHltbSettings(
        await providerSettingsService.setHltbSettings({
          ...hltbSettings,
          [key]: value,
        }),
      );
    } catch (error) {
      setHltbError(toHltbErrorMessage(error));
    }
  };

  const startHltbSync = async (onlyMissing: boolean) => {
    if (hltbSyncStarting) return;
    setHltbSyncStarting(true);
    setHltbError(null);
    try {
      setHltbSyncStatus(
        await providerSettingsService.syncHltbLibrary(onlyMissing),
      );
      await queryClient.invalidateQueries({ queryKey: ["games"] });
    } catch (error) {
      setHltbError(toHltbErrorMessage(error));
      try {
        setHltbSyncStatus(await providerSettingsService.getHltbSyncStatus());
      } catch {
        // Preserve the original error when status polling is unavailable.
      }
    } finally {
      setHltbSyncStarting(false);
    }
  };

  const cancelHltbSync = async () => {
    if (hltbSyncStatus?.status !== "running") return;
    try {
      setHltbSyncStatus(await providerSettingsService.cancelHltbSync());
    } catch (error) {
      setHltbError(toHltbErrorMessage(error));
    }
  };

  return (
    <section className="settings-view" aria-labelledby="settings-heading">
      {level === "settings" && (
        <SettingsHome
          onOpenDisplay={() => onLevelChange("display")}
          onOpenNetwork={() => onLevelChange("network")}
          onOpenBluetooth={() => onLevelChange("bluetooth")}
          onOpenIntegrations={() => onLevelChange("integrations")}
          onOpenStorage={() => onLevelChange("storage")}
          onOpenLibrary={() => onLevelChange("library")}
          onAvailability={setAvailabilityFeedback}
        />
      )}
      {level === "display" && <DisplayView />}
      {level === "network" && <NetworkView />}
      {level === "bluetooth" && (
        <BluetoothErrorBoundary>
          <BluetoothView />
        </BluetoothErrorBoundary>
      )}
      {level === "library" && (
        <HiddenGamesView
          games={games}
          onRestore={(gameId) => {
            void providerSettingsService
              .setGameHidden(gameId, false)
              .then(() => {
                queryClient.setQueryData<Game[]>(["games"], (currentGames) =>
                  currentGames?.map((game) =>
                    game.id === gameId ? { ...game, hidden: false } : game,
                  ),
                );
              })
              .catch(() => undefined);
          }}
        />
      )}
      {level === "integrations" && (
        <IntegrationsView
          onOpenEden={() => onLevelChange("eden")}
          onOpenLaunchBox={() => onLevelChange("launchbox")}
          onOpenSteam={() => onLevelChange("steam")}
          onOpenHltb={() => onLevelChange("hltb")}
          onOpenSteamGridDb={() => onLevelChange("steamgriddb")}
          onOpenRapidApiReviews={() => onLevelChange("rapidapi-reviews")}
          onOpenAIServices={() => onLevelChange("ai-services")}
          onOpenLosslessScaling={() => onLevelChange("lossless-scaling")}
          onAvailability={setAvailabilityFeedback}
        />
      )}
      {level === "eden" && (
        <EdenView
          status={edenStatus}
          inspection={edenInspection}
          executablePath={edenExecutableDraft}
          manualRoots={edenManualRoots}
          loading={edenLoading}
          errorMessage={edenError}
          onSelectExecutable={() => void selectEdenExecutable()}
          onSelectGameFolder={() => void selectEdenGameFolder()}
          onConnect={() => void connectEden()}
          onRescan={() => void rescanEden()}
          onDisconnect={() => void disconnectEden()}
        />
      )}
      {level === "launchbox" && (
        <LaunchBoxView
          status={launchBoxStatus}
          loading={launchBoxLoading}
          updating={launchBoxUpdating}
          errorMessage={launchBoxError}
          notice={launchBoxNotice}
          onRetry={() => void loadLaunchBoxStatus()}
          onUpdate={() => void updateLaunchBoxCatalog()}
        />
      )}
      {level === "lossless-scaling" && (
        <LosslessScalingView
          status={losslessScalingStatus}
          errorMessage={losslessScalingError}
          onOpenApplication={() =>
            void frameGenerationService
              .openApplication()
              .then(() =>
                frameGenerationService
                  .getStatus()
                  .then(setLosslessScalingStatus),
              )
              .catch(() =>
                setLosslessScalingError("No se pudo abrir Lossless Scaling."),
              )
          }
          onRestoreBackup={() =>
            void frameGenerationService
              .restoreBackup()
              .then(() =>
                frameGenerationService
                  .getStatus()
                  .then(setLosslessScalingStatus),
              )
              .catch(() =>
                setLosslessScalingError("No se pudo restaurar el backup."),
              )
          }
          onRestartApplication={() =>
            void frameGenerationService
              .restartApplication()
              .then(() =>
                frameGenerationService
                  .getStatus()
                  .then(setLosslessScalingStatus),
              )
              .catch(() =>
                setLosslessScalingError(
                  "No se pudo reiniciar Lossless Scaling de forma segura.",
                ),
              )
          }
        />
      )}
      {level === "steamgriddb" && (
        <SteamGridDbView
          configuration={steamGridDbConfiguration}
          apiKeyDraft={steamGridDbApiKeyDraft}
          editing={steamGridDbEditing}
          saving={steamGridDbSaving}
          errorMessage={steamGridDbError}
          deleteConfirm={steamGridDbDeleteConfirm}
          onApiKeyDraftChange={setSteamGridDbApiKeyDraft}
          onOpenApiKey={openSteamGridDbApiKeyEditor}
          onSaveApiKey={() => void saveSteamGridDbApiKey()}
          onCancelEdit={handleBack}
          onOpenDelete={openSteamGridDbDeleteDialog}
          onCancelDelete={() => setSteamGridDbDeleteConfirm(false)}
          onConfirmDelete={() => void deleteSteamGridDbApiKey()}
        />
      )}
      {level === "rapidapi-reviews" && (
        <RapidApiReviewsView
          configuration={rapidApiReviewsConfiguration}
          apiKeyDraft={rapidApiReviewsApiKeyDraft}
          editing={rapidApiReviewsEditing}
          saving={rapidApiReviewsSaving}
          errorMessage={rapidApiReviewsError}
          deleteConfirm={rapidApiReviewsDeleteConfirm}
          onApiKeyDraftChange={setRapidApiReviewsApiKeyDraft}
          onOpenApiKey={openRapidApiReviewsApiKeyEditor}
          onSaveApiKey={() => void saveRapidApiReviewsApiKey()}
          onCancelEdit={handleBack}
          onOpenDelete={openRapidApiReviewsDeleteDialog}
          onCancelDelete={() => setRapidApiReviewsDeleteConfirm(false)}
          onConfirmDelete={() => void deleteRapidApiReviewsApiKey()}
        />
      )}
      {level === "ai-services" && (
        <AIServicesView
          configuration={aiConfiguration}
          apiKeyDraft={aiApiKeyDraft}
          modelDraft={aiModelDraft}
          connectionStatus={aiConnectionStatus}
          saving={aiSaving}
          testing={aiTesting}
          errorMessage={aiError}
          onApiKeyDraftChange={setAiApiKeyDraft}
          onModelDraftChange={setAiModelDraft}
          onSave={() => void saveAIConfiguration()}
          onTestConnection={() => void testAIConnection()}
        />
      )}
      {level === "hltb" && (
        <HltbView
          settings={hltbSettings}
          status={hltbSyncStatus}
          errorMessage={hltbError}
          syncStarting={hltbSyncStarting}
          onToggle={(key, value) => void updateHltbSetting(key, value)}
          onStartSync={(onlyMissing) => void startHltbSync(onlyMissing)}
          onCancelSync={() => void cancelHltbSync()}
        />
      )}
      {level === "steam" && (
        <SteamView
          configuration={configuration}
          configurationState={configurationState}
          profile={profile}
          profileState={profileState}
          profileError={profileError}
          steamIdDraft={steamIdDraft}
          apiKeyDraft={apiKeyDraft}
          editingSteamId={editingSteamId}
          editingApiKey={editingApiKey}
          errorMessage={errorMessage}
          confirmDisconnect={confirmDisconnect}
          onSteamIdDraftChange={setSteamIdDraft}
          onApiKeyDraftChange={setApiKeyDraft}
          onOpenSteamId={openSteamIdEditor}
          onOpenApiKey={openApiKeyEditor}
          onSaveSteamId={() => void saveSteamId()}
          onSaveApiKey={() => void saveApiKey()}
          onCancelEdit={handleBack}
          onOpenDisconnect={openDisconnectDialog}
          onCancelDisconnect={() => setConfirmDisconnect(false)}
          onConfirmDisconnect={() => void disconnect()}
          syncStatus={syncStatus}
          syncScope={syncScope}
          syncScopeSaving={syncScopeSaving}
          syncStarting={syncStarting}
          achievementSyncStatus={achievementSyncStatus}
          achievementSyncStarting={achievementSyncStarting}
          imageSyncStatus={imageSyncStatus}
          imageSyncStarting={imageSyncStarting}
          onStartSync={() => void startSteamSync()}
          onSetSyncScope={(scope) => void saveSteamSyncScope(scope)}
          onCancelSync={() => void cancelSteamSync()}
          onStartAchievementSync={() => void startSteamAchievementSync()}
          onStartImageSync={() => void startSteamImageSync()}
          onCancelImageSync={() => void cancelSteamImageSync()}
        />
      )}
      {level === "storage" && (
        <StorageView
          status={storageStatus}
          loading={storageLoading}
          errorMessage={storageError}
          deleteSource={storageDeleteSource}
          onToggleDeleteSource={() => setStorageDeleteSource((value) => !value)}
          onOpenMigration={openStorageMigrationDialog}
        />
      )}
      {storageConfirm && storageStatus && (
        <StorageMigrationDialog
          status={storageStatus}
          deleteSource={storageDeleteSource}
          onCancel={() => setStorageConfirm(false)}
          onConfirm={() => void startStorageMigration()}
        />
      )}
      {availabilityFeedback && (
        <AvailabilityFeedback
          availability={availabilityFeedback}
          onDismiss={() => setAvailabilityFeedback(null)}
        />
      )}
    </section>
  );
}

function SettingsHome({
  onOpenDisplay,
  onOpenNetwork,
  onOpenBluetooth,
  onOpenIntegrations,
  onOpenStorage,
  onOpenLibrary,
  onAvailability,
}: {
  onOpenDisplay: () => void;
  onOpenNetwork: () => void;
  onOpenBluetooth: () => void;
  onOpenIntegrations: () => void;
  onOpenStorage: () => void;
  onOpenLibrary: () => void;
  onAvailability: (
    availability: Exclude<ActionAvailability, "available" | "unavailable">,
  ) => void;
}) {
  return (
    <>
      <SettingsHeading
        eyebrow="Preferencias"
        title="Configuración"
        description="Personaliza tu experiencia en LumaDeck"
      />
      <NavigationGrid
        groupId="settings-items"
        columns={1}
        itemCount={SETTINGS_ITEMS.length}
        regionId="settings-content"
        entryFocusId="settings-integrations"
        exitFocusId="main-nav-settings"
        className="settings-card-grid"
      >
        {SETTINGS_ITEMS.map(([id, title, description, icon], index) => {
          const availability = settingsAvailability(id);
          const enabled = availability === "available";
          return (
            <AvailabilityFocusable
              key={id}
              focusId={`settings-${id}`}
              scopeId="settings-shell"
              gridIndex={index}
              availability={availability}
              className={`settings-card ${enabled ? "is-enabled" : "is-coming-soon"}`}
              onAvailable={
                enabled
                  ? id === "display"
                    ? onOpenDisplay
                    : id === "network"
                      ? onOpenNetwork
                      : id === "bluetooth"
                        ? onOpenBluetooth
                        : id === "storage"
                          ? onOpenStorage
                          : id === "library"
                            ? onOpenLibrary
                            : onOpenIntegrations
                  : undefined
              }
              onAvailabilityFeedback={onAvailability}
            >
              <span className="settings-card-icon" aria-hidden="true">
                {icon}
              </span>
              <span className="settings-card-copy">
                <strong>{title}</strong>
                <small>
                  {enabled ? description : `${description} · Próximamente`}
                </small>
              </span>
              <span className="settings-card-arrow">›</span>
            </AvailabilityFocusable>
          );
        })}
      </NavigationGrid>
    </>
  );
}

function HiddenGamesView({
  games,
  onRestore,
}: {
  games: readonly Game[];
  onRestore: (gameId: string) => void;
}) {
  const hiddenGames = games.filter((game) => game.hidden === true);

  return (
    <>
      <SettingsHeading
        eyebrow="Configuración · Biblioteca"
        title="Juegos ocultos"
        description="Estos juegos conservan todos sus datos y no aparecen en la experiencia normal."
      />
      {hiddenGames.length === 0 ? (
        <p className="empty-state">No hay juegos ocultos.</p>
      ) : (
        <NavigationGrid
          groupId="hidden-games"
          columns={1}
          itemCount={hiddenGames.length}
          regionId="settings-content"
          entryFocusId={`hidden-game-${hiddenGames[0]?.id ?? "empty"}`}
          exitFocusId="settings-library"
          className="settings-card-grid"
        >
          {hiddenGames.map((game, index) => (
            <Focusable
              key={game.id}
              focusId={`hidden-game-${game.id}`}
              scopeId="settings-shell"
              gridIndex={index}
              className="settings-card is-enabled"
              onConfirm={() => onRestore(game.id)}
              aria-label={`Mostrar ${game.title}`}
            >
              <span className="settings-card-copy">
                <strong>{game.title}</strong>
                <small>
                  {game.provider} · {game.platform}
                </small>
              </span>
              <span className="settings-card-arrow">Mostrar</span>
            </Focusable>
          ))}
        </NavigationGrid>
      )}
    </>
  );
}

function IntegrationsView({
  onOpenEden,
  onOpenLaunchBox,
  onOpenSteam,
  onOpenHltb,
  onOpenSteamGridDb,
  onOpenRapidApiReviews,
  onOpenAIServices,
  onOpenLosslessScaling,
  onAvailability,
}: {
  onOpenEden: () => void;
  onOpenLaunchBox: () => void;
  onOpenSteam: () => void;
  onOpenHltb: () => void;
  onOpenSteamGridDb: () => void;
  onOpenRapidApiReviews: () => void;
  onOpenAIServices: () => void;
  onOpenLosslessScaling: () => void;
  onAvailability: (
    availability: Exclude<ActionAvailability, "available" | "unavailable">,
  ) => void;
}) {
  return (
    <>
      <SettingsHeading
        eyebrow="Configuración · Integraciones"
        title="Integraciones"
        description="Gestiona tus cuentas y servicios"
      />
      <NavigationGrid
        groupId="integration-providers"
        columns={1}
        itemCount={PROVIDERS.length}
        regionId="settings-content"
        entryFocusId="integration-steam"
        exitFocusId="settings-integrations"
        className="settings-card-grid"
      >
        {PROVIDERS.map(([id, title, description, icon], index) => {
          const availability = providerAvailability(id);
          const enabled = availability === "available";
          return (
            <AvailabilityFocusable
              key={id}
              focusId={`integration-${id}`}
              scopeId="settings-shell"
              gridIndex={index}
              availability={availability}
              className={`provider-card ${enabled ? "is-enabled" : "is-coming-soon"}`}
              onAvailable={
                enabled
                  ? id === "eden"
                    ? onOpenEden
                    : id === "launchbox"
                      ? onOpenLaunchBox
                      : id === "steam"
                        ? onOpenSteam
                        : id === "hltb"
                          ? onOpenHltb
                          : id === "steamgriddb"
                            ? onOpenSteamGridDb
                            : id === "rapidapi-reviews"
                              ? onOpenRapidApiReviews
                              : id === "ai-services"
                                ? onOpenAIServices
                                : onOpenLosslessScaling
                  : undefined
              }
              onAvailabilityFeedback={onAvailability}
            >
              <ProviderIcon providerId={id} fallback={icon} />
              <span className="settings-card-copy">
                <strong>{title}</strong>
                <small>{description}</small>
              </span>
              <span
                className={`provider-status ${enabled ? "is-ready" : "is-soon"}`}
              >
                {enabled ? "Configurar" : "Próximamente"}
              </span>
            </AvailabilityFocusable>
          );
        })}
      </NavigationGrid>
    </>
  );
}

function formatLaunchBoxBytes(value: number | null): string | null {
  if (value === null || !Number.isFinite(value)) return null;
  if (value < 1024 * 1024) return `${Math.round(value / 1024)} KB`;
  return `${(value / (1024 * 1024)).toFixed(1)} MB`;
}

function formatLaunchBoxDate(value: string | null): string | null {
  if (!value) return null;
  const timestamp = Number(value);
  const date = Number.isFinite(timestamp)
    ? new Date(timestamp * 1000)
    : new Date(value);
  if (Number.isNaN(date.getTime())) return null;
  return new Intl.DateTimeFormat("es", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

function launchBoxPhaseCopy(progress: LaunchBoxCatalogProgress | null) {
  switch (progress?.phase) {
    case "extracting":
      return {
        title: "Preparando catálogo",
        description: "Extrayendo archivos de metadatos…",
      };
    case "importing":
      return {
        title: "Importando metadatos",
        description: "Procesando juegos del catálogo local…",
      };
    case "validating":
      return {
        title: "Validando catálogo",
        description: "Comprobando la integridad de los datos importados…",
      };
    case "activating":
      return {
        title: "Activando catálogo",
        description: "Preparando la nueva versión para LumaDeck…",
      };
    case "downloading":
    default:
      return {
        title: "Descargando catálogo",
        description: "Descargando la fuente oficial de metadatos…",
      };
  }
}

function formatLaunchBoxDuration(milliseconds: number): string {
  const seconds = Math.max(0, Math.floor(milliseconds / 1000));
  const minutes = Math.floor(seconds / 60);
  return minutes > 0
    ? `${minutes}m ${String(seconds % 60).padStart(2, "0")}s`
    : `${seconds}s`;
}

function LaunchBoxView({
  status,
  loading,
  updating,
  errorMessage,
  notice,
  onRetry,
  onUpdate,
}: {
  status: LaunchBoxCatalogStatus | null;
  loading: boolean;
  updating: boolean;
  errorMessage: string | null;
  notice: string | null;
  onRetry: () => void;
  onUpdate: () => void;
}) {
  const catalogUpdating = updating || status?.status === "updating";
  const ready = status?.available === true && !catalogUpdating;
  const hasStatus = status !== null;
  const lastUpdate = formatLaunchBoxDate(status?.downloadedAt ?? null);
  const nextUpdate = formatLaunchBoxDate(status?.expiresAt ?? null);
  const zipSize = formatLaunchBoxBytes(status?.zipSizeBytes ?? null);
  const sourceSize = formatLaunchBoxBytes(status?.sourceSizeBytes ?? null);
  const progress = status?.progress ?? null;
  const phaseCopy = launchBoxPhaseCopy(progress);
  const progressTotal =
    progress?.phase === "downloading"
      ? progress.totalBytes
      : progress?.totalRecords;
  const progressValue =
    progress?.phase === "downloading"
      ? progress.downloadedBytes
      : progress?.processedRecords;
  const progressPercent = launchBoxProgressPercent(progress);
  const lastProgressAgo = progress
    ? Math.max(0, Math.floor((Date.now() - progress.lastProgressAtMs) / 1000))
    : null;
  const lastErrorMessage = status?.lastError
    ? (launchBoxErrorMessage(status.lastError) ??
      "No se pudo actualizar el catálogo. Se seguirá utilizando la versión anterior.")
    : null;
  const actionLabel = ready ? "Actualizar catálogo" : "Descargar catálogo";

  return (
    <>
      <SettingsHeading
        eyebrow="Configuración · Integraciones"
        title="LaunchBox"
        description="Catálogo local de metadatos y capturas para juegos emulados"
      />
      <div className="eden-settings-layout">
        <article className="settings-panel">
          <div className="settings-panel-heading">
            <ProviderIcon providerId="launchbox" fallback="L" />
            <div>
              <p className="eyebrow">Catálogo local</p>
              <h2>
                {catalogUpdating
                  ? phaseCopy.title
                  : ready
                    ? "Catálogo disponible"
                    : "Catálogo no descargado"}
              </h2>
            </div>
          </div>
          {!hasStatus && loading && (
            <p className="settings-muted">Consultando estado del catálogo…</p>
          )}
          {catalogUpdating && (
            <div
              className="launchbox-progress"
              role="status"
              aria-live="polite"
            >
              <div className="launchbox-progress-heading">
                <strong>{phaseCopy.title}</strong>
                {progressPercent !== null && (
                  <strong>{progressPercent}%</strong>
                )}
              </div>
              <p>{phaseCopy.description}</p>
              <div
                className={`launchbox-progress-track ${progressPercent === null ? "is-indeterminate" : ""}`}
                role="progressbar"
                aria-label={phaseCopy.title}
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={progressPercent ?? undefined}
              >
                {progressPercent !== null && (
                  <span style={{ width: `${progressPercent}%` }} />
                )}
              </div>
              {progress?.phase === "downloading" && progressValue !== null && (
                <div className="launchbox-progress-summary">
                  <span>
                    {formatLaunchBoxBytes(progressValue ?? null)}
                    {progressTotal !== null && progressTotal !== undefined
                      ? ` / ${formatLaunchBoxBytes(progressTotal)}`
                      : " descargados"}
                  </span>
                  <span>{formatLaunchBoxDuration(progress.elapsedMs)}</span>
                </div>
              )}
              {progress?.phase !== "downloading" && progress && (
                <div className="launchbox-progress-summary">
                  <span>
                    {progress.processedRecords.toLocaleString("es")} juegos
                    procesados
                  </span>
                  <span>{formatLaunchBoxDuration(progress.elapsedMs)}</span>
                </div>
              )}
              {lastProgressAgo !== null && (
                <small>Último progreso: hace {lastProgressAgo}s</small>
              )}
            </div>
          )}
          {!ready && hasStatus && !catalogUpdating && (
            <p className="settings-muted">
              Descarga el catálogo para enriquecer los juegos emulados. Esta
              acción no descarga capturas en bloque.
            </p>
          )}
          {ready && (
            <div className="settings-detail-list">
              {lastUpdate && (
                <div className="settings-detail-row">
                  <span>Última actualización</span>
                  <strong>{lastUpdate}</strong>
                </div>
              )}
              {nextUpdate && (
                <div className="settings-detail-row">
                  <span>Próxima actualización</span>
                  <strong>{nextUpdate}</strong>
                </div>
              )}
              <div className="settings-detail-row">
                <span>Actualización automática</span>
                <strong>Cada 30 días</strong>
              </div>
              {status.recordCount > 0 && (
                <div className="settings-detail-row">
                  <span>Juegos</span>
                  <strong>{status.recordCount.toLocaleString("es")}</strong>
                </div>
              )}
              {status.switchRecordCount > 0 && (
                <div className="settings-detail-row">
                  <span>Nintendo Switch</span>
                  <strong>
                    {status.switchRecordCount.toLocaleString("es")}
                  </strong>
                </div>
              )}
              {zipSize && (
                <div className="settings-detail-row">
                  <span>Tamaño local</span>
                  <strong>{zipSize}</strong>
                </div>
              )}
              {sourceSize && (
                <div className="settings-detail-row">
                  <span>Metadatos sin comprimir</span>
                  <strong>{sourceSize}</strong>
                </div>
              )}
            </div>
          )}
          {ready && lastErrorMessage && (
            <p className="settings-feedback is-error" role="alert">
              La última actualización falló; el catálogo disponible sigue
              utilizándose. Última fase: {phaseCopy.title}. {lastErrorMessage}
            </p>
          )}
          {status?.status === "error" && lastErrorMessage && (
            <p className="settings-feedback is-error" role="alert">
              No se pudo actualizar el catálogo. Última fase: {phaseCopy.title}.{" "}
              {lastErrorMessage}
            </p>
          )}
          {errorMessage && (
            <p className="settings-feedback is-error" role="alert">
              {errorMessage}
            </p>
          )}
          {notice && (
            <p className="settings-feedback is-success" role="status">
              {notice}
            </p>
          )}
          {!catalogUpdating && (
            <Focusable
              focusId="launchbox-catalog-action"
              scopeId="settings-shell"
              className="settings-button primary"
              disabled={loading}
              onConfirm={onUpdate}
            >
              {actionLabel}
            </Focusable>
          )}
          {errorMessage && !ready && !updating && (
            <Focusable
              focusId="launchbox-catalog-retry"
              scopeId="settings-shell"
              className="settings-button secondary"
              onConfirm={onRetry}
            >
              Reintentar
            </Focusable>
          )}
        </article>
        <aside className="settings-panel settings-security-note">
          <strong>Capacidades</strong>
          <p>
            Metadatos, géneros, multiplayer local, valoración comunitaria y
            capturas de gameplay.
          </p>
          <small>
            Las capturas se resuelven por juego cuando abres Details. El arte de
            biblioteca continúa usando SteamGridDB; LaunchBox solo actúa como
            fallback.
          </small>
        </aside>
      </div>
    </>
  );
}

function EdenView({
  status,
  inspection,
  executablePath,
  manualRoots,
  loading,
  errorMessage,
  onSelectExecutable,
  onSelectGameFolder,
  onConnect,
  onRescan,
  onDisconnect,
}: {
  status: EdenStatus | null;
  inspection: EdenExecutableInspection | null;
  executablePath: string;
  manualRoots: string[];
  loading: boolean;
  errorMessage: string | null;
  onSelectExecutable: () => void;
  onSelectGameFolder: () => void;
  onConnect: () => void;
  onRescan: () => void;
  onDisconnect: () => void;
}) {
  const connected = status?.status === "ready";
  const roots = status?.libraryRoots ?? inspection?.libraryRoots ?? [];
  const profiles = status?.profiles ?? inspection?.profiles ?? [];
  const currentProfile = profiles.find((profile) => profile.isCurrent);
  const games = status?.gamesDetected ?? 0;
  return (
    <>
      <SettingsHeading
        eyebrow="Configuración · Integraciones"
        title="Eden"
        description="Nintendo Switch · discovery local de juegos"
      />
      <div className="eden-settings-layout">
        <div className="eden-settings-main">
          <article className="settings-panel eden-profile-panel">
            <div className="eden-account-heading">
              <ProviderIcon providerId="eden" fallback="E" />
              <div>
                <p className="eyebrow">Cuenta local</p>
                <h2>Eden</h2>
              </div>
              <span className="eden-config-status">
                {connected ? "Instalación verificada" : "No configurado"}
              </span>
            </div>
            {profiles.length > 0 && (
              <div className="eden-profile-list">
                <p className="eyebrow">Perfiles detectados</p>
                {profiles.map((profile) => (
                  <div key={profile.id} className="eden-profile-list-item">
                    {profile.avatarDataUrl ? (
                      <img
                        className="eden-profile-list-avatar"
                        src={profile.avatarDataUrl}
                        alt=""
                      />
                    ) : (
                      <div className="eden-profile-list-avatar eden-profile-avatar-fallback">
                        E
                      </div>
                    )}
                    <div>
                      <strong>{profile.name}</strong>
                      <small>{profile.id}</small>
                    </div>
                    {profile.isCurrent && (
                      <span className="eden-profile-current">Activo</span>
                    )}
                  </div>
                ))}
              </div>
            )}
            <div className="eden-summary-row">
              <span>Perfil activo</span>
              <strong>{currentProfile?.name ?? "No detectado"}</strong>
            </div>
            <div className="eden-summary-row">
              <span>UUID</span>
              <strong>{currentProfile?.id ?? "No disponible"}</strong>
            </div>
            <div className="eden-summary-row">
              <span>Perfiles</span>
              <strong>{profiles.length}</strong>
            </div>
            <div className="eden-summary-row">
              <span>Juegos</span>
              <strong>{games}</strong>
            </div>
          </article>
          <div className="settings-detail-panel eden-settings-panel">
            <div className="settings-detail-row">
              <span>Estado</span>
              <strong>
                {connected ? `Conectado · ${games} juegos` : "No configurado"}
              </strong>
            </div>
            <div className="settings-detail-row">
              <span>Executable</span>
              <strong className="settings-detail-value">
                {executablePath || "Selecciona eden.exe"}
              </strong>
            </div>
            {inspection && (
              <>
                <div className="settings-detail-row">
                  <span>Configuration</span>
                  <strong>
                    {inspection.configurationFound
                      ? "✓ Encontrada"
                      : "No encontrada"}
                  </strong>
                </div>
                <div className="settings-detail-row">
                  <span>Game folders</span>
                  <strong>{roots.length} detectadas</strong>
                </div>
              </>
            )}
            {roots.length > 0 && (
              <div className="settings-detail-list">
                {roots.map((root) => (
                  <div key={root.path} className="settings-detail-list-item">
                    <span>{root.path}</span>
                    <small>
                      {"available" in root
                        ? root.available
                          ? "gameCount" in root
                            ? `${root.gameCount} juegos`
                            : "Disponible"
                          : "No disponible"
                        : root.deepScan
                          ? "Recursivo"
                          : "Directo"}
                    </small>
                  </div>
                ))}
              </div>
            )}
            {manualRoots.length > 0 && (
              <p className="settings-inline-note">
                Fallback manual: {manualRoots.join(", ")}
              </p>
            )}
            {errorMessage && <p className="settings-error">{errorMessage}</p>}
            {status?.warnings.map((warning) => (
              <p key={warning} className="settings-inline-note">
                {warning}
              </p>
            ))}
          </div>
        </div>
        <aside className="settings-panel eden-settings-aside">
          <h2>Configuración local</h2>
          <p>
            Eden mantiene los perfiles, avatares y partidas guardadas en su
            carpeta local. LumaDeck solo los lee para mostrarlos.
          </p>
          <div className="eden-aside-divider" />
          <p>
            La selección y creación de perfiles continúa en Eden para proteger
            los datos de guardado.
          </p>
          <div className="eden-aside-stat">
            <span>Perfil actual</span>
            <strong>{currentProfile?.name ?? "No detectado"}</strong>
          </div>
        </aside>
      </div>
      <NavigationGrid
        groupId="eden-actions"
        columns={1}
        itemCount={connected ? 3 : 3}
        regionId="settings-content"
        entryFocusId={connected ? "eden-rescan" : "eden-select-executable"}
        exitFocusId="integration-eden"
        className="settings-card-grid eden-actions-grid"
      >
        {!connected && (
          <Focusable
            focusId="eden-select-executable"
            scopeId="settings-shell"
            gridIndex={0}
            className="settings-card is-enabled"
            onConfirm={onSelectExecutable}
          >
            <span className="settings-card-copy">
              <strong>Select Eden executable</strong>
              <small>Selecciona únicamente eden.exe</small>
            </span>
            <span className="settings-card-arrow">Seleccionar</span>
          </Focusable>
        )}
        {!connected && inspection && (
          <Focusable
            focusId="eden-connect"
            scopeId="settings-shell"
            gridIndex={1}
            className="settings-card is-enabled"
            onConfirm={onConnect}
          >
            <span className="settings-card-copy">
              <strong>Connect</strong>
              <small>Importar juegos detectados a Library</small>
            </span>
            <span className="settings-card-arrow">
              {loading ? "…" : "Conectar"}
            </span>
          </Focusable>
        )}
        {!connected && (
          <Focusable
            focusId="eden-manual-folder"
            scopeId="settings-shell"
            gridIndex={2}
            className="settings-card is-enabled"
            onConfirm={onSelectGameFolder}
          >
            <span className="settings-card-copy">
              <strong>Select game folder manually</strong>
              <small>Fallback si Eden no expone sus roots</small>
            </span>
            <span className="settings-card-arrow">Añadir</span>
          </Focusable>
        )}
        {connected && (
          <>
            <Focusable
              focusId="eden-rescan"
              scopeId="settings-shell"
              gridIndex={0}
              className="settings-card is-enabled"
              onConfirm={onRescan}
            >
              <span className="settings-card-copy">
                <strong>Rescan</strong>
                <small>Volver a leer configuración y juegos</small>
              </span>
              <span className="settings-card-arrow">
                {loading ? "…" : "Actualizar"}
              </span>
            </Focusable>
            <Focusable
              focusId="eden-configure"
              scopeId="settings-shell"
              gridIndex={1}
              className="settings-card is-enabled"
              onConfirm={onSelectExecutable}
            >
              <span className="settings-card-copy">
                <strong>Configure</strong>
                <small>Cambiar ejecutable de Eden</small>
              </span>
              <span className="settings-card-arrow">Cambiar</span>
            </Focusable>
            <Focusable
              focusId="eden-disconnect"
              scopeId="settings-shell"
              gridIndex={2}
              className="settings-card is-enabled"
              onConfirm={onDisconnect}
            >
              <span className="settings-card-copy">
                <strong>Disconnect</strong>
                <small>Conserva los juegos importados</small>
              </span>
              <span className="settings-card-arrow">Desconectar</span>
            </Focusable>
          </>
        )}
      </NavigationGrid>
    </>
  );
}

function ProviderIcon({
  providerId,
  fallback,
}: {
  providerId: string;
  fallback: string;
}) {
  const imageSource =
    providerId === "eden"
      ? edenLogo
      : providerId === "launchbox"
        ? launchboxLogo
        : providerId === "lossless-scaling"
          ? losslessScalingLogo
          : providerId === "steam"
            ? steamLogo
            : providerId === "steamgriddb"
              ? steamGridDbLogo
              : providerId === "rapidapi-reviews"
                ? opencriticLogo
                : undefined;

  return (
    <span
      className={`provider-icon ${imageSource ? "provider-icon-image" : ""}`}
      aria-hidden="true"
    >
      {imageSource ? <img src={imageSource} alt="" /> : fallback}
    </span>
  );
}

function LosslessScalingView({
  status,
  errorMessage,
  onOpenApplication,
  onRestoreBackup,
  onRestartApplication,
}: {
  status: LosslessScalingStatus | null;
  errorMessage: string | null;
  onOpenApplication: () => void;
  onRestoreBackup: () => void;
  onRestartApplication: () => void;
}) {
  return (
    <>
      <SettingsHeading
        eyebrow="Configuración · Integraciones"
        title="Lossless Scaling"
        description="Estado de la integración de Frame Generation"
      />
      <div className="steam-settings-layout">
        <div className="steam-settings-main">
          <article className="settings-panel steam-account-panel">
            <div className="steam-sync-heading">
              <div>
                <p className="eyebrow">Proveedor</p>
                <h2>{status?.status ?? "Consultando…"}</h2>
              </div>
              <span className="steam-sync-status is-completed">
                {status?.applicationRunning ? "Abierto" : "Cerrado"}
              </span>
            </div>
            <div className="steam-summary-row">
              <span>Versión</span>
              <strong>{status?.version ?? "Consultando…"}</strong>
            </div>
            <div className="steam-summary-row">
              <span>Installation path</span>
              <strong>{status?.installationPath ?? "No disponible"}</strong>
            </div>
            <div className="steam-summary-row">
              <span>Settings.xml</span>
              <strong>{status?.settingsStatus ?? "Consultando…"}</strong>
            </div>
            <div className="settings-action-row">
              <Focusable
                focusId="lossless-scaling-open"
                scopeId="settings-shell"
                className="settings-button primary"
                onConfirm={onOpenApplication}
              >
                Open application
              </Focusable>
              <Focusable
                focusId="lossless-scaling-restore-backup"
                scopeId="settings-shell"
                className="settings-button secondary"
                onConfirm={onRestoreBackup}
              >
                Restore backup
              </Focusable>
              {status?.applicationRunning && (
                <Focusable
                  focusId="lossless-scaling-restart"
                  scopeId="settings-shell"
                  className="settings-button secondary"
                  onConfirm={onRestartApplication}
                >
                  Restart Lossless Scaling
                </Focusable>
              )}
            </div>
          </article>
        </div>
      </div>
      {errorMessage && (
        <p className="settings-feedback is-error" role="alert">
          {errorMessage}
        </p>
      )}
    </>
  );
}

function SteamGridDbView({
  configuration,
  apiKeyDraft,
  editing,
  saving,
  errorMessage,
  deleteConfirm,
  onApiKeyDraftChange,
  onOpenApiKey,
  onSaveApiKey,
  onCancelEdit,
  onOpenDelete,
  onCancelDelete,
  onConfirmDelete,
}: {
  configuration: SteamGridDbConfigurationStatus | null;
  apiKeyDraft: string;
  editing: boolean;
  saving: boolean;
  errorMessage: string | null;
  deleteConfirm: boolean;
  onApiKeyDraftChange: (value: string) => void;
  onOpenApiKey: () => void;
  onSaveApiKey: () => void;
  onCancelEdit: () => void;
  onOpenDelete: () => void;
  onCancelDelete: () => void;
  onConfirmDelete: () => void;
}) {
  const configured = configuration?.apiKeyConfigured ?? false;
  return (
    <>
      <SettingsHeading
        eyebrow="Configuración · Integraciones · SteamGridDB"
        title="SteamGridDB"
        description="Obtén portadas, fondos, logos e iconos para personalizar tu biblioteca."
      />
      <div className="steam-settings-layout">
        <div className="steam-settings-main">
          <article className="settings-panel">
            <div className="steam-account-heading">
              <ProviderIcon providerId="steamgriddb" fallback="▦" />
              <div>
                <p className="eyebrow">Proveedor de recursos gráficos</p>
                <h2>SteamGridDB</h2>
              </div>
              <strong
                className={`steam-config-status status-${configuration?.status ?? "not-configured"}`}
              >
                {steamGridDbStatusLabel(configuration?.status)}
              </strong>
            </div>
            <div className="steam-summary-row">
              <span>Tipo</span>
              <strong>Proveedor de arte</strong>
            </div>
            <div className="steam-summary-row">
              <span>API Key</span>
              <strong>{configuration?.apiKeyMasked ?? "No configurada"}</strong>
            </div>
          </article>
          {editing || !configured ? (
            <article className="settings-panel settings-editor-panel">
              <p className="eyebrow">SteamGridDB API Key</p>
              <p className="settings-helper">
                Se guardará cifrada con DPAPI de Windows y nunca se mostrará
                completa.
              </p>
              <GamepadTextInput
                focusId="steamgriddb-api-key-input"
                scopeId="settings-shell"
                value={apiKeyDraft}
                onChange={onApiKeyDraftChange}
                placeholder="Ingresa tu API Key"
                ariaLabel="SteamGridDB API Key"
                secure
                maxLength={256}
                className="settings-input"
              />
              <div className="settings-action-row">
                <Focusable
                  focusId="steamgriddb-api-key-save"
                  scopeId="settings-shell"
                  className="settings-button primary"
                  disabled={saving}
                  onConfirm={onSaveApiKey}
                >
                  {saving ? "Guardando…" : "Guardar API Key"}
                </Focusable>
                {editing && (
                  <Focusable
                    focusId="steamgriddb-api-key-cancel"
                    scopeId="settings-shell"
                    className="settings-button secondary"
                    onConfirm={onCancelEdit}
                  >
                    Cancelar
                  </Focusable>
                )}
              </div>
            </article>
          ) : (
            <Focusable
              focusId="steamgriddb-change-key"
              scopeId="settings-shell"
              className="settings-action settings-action-primary"
              onConfirm={onOpenApiKey}
            >
              Cambiar API Key
            </Focusable>
          )}
          {configured && (
            <Focusable
              focusId="steamgriddb-delete-key"
              scopeId="settings-shell"
              className="settings-action settings-action-danger"
              onConfirm={onOpenDelete}
            >
              Eliminar API Key
            </Focusable>
          )}
          {errorMessage && (
            <p className="settings-feedback is-error" role="alert">
              {errorMessage}
            </p>
          )}
        </div>
        <aside className="settings-panel settings-security-note">
          <strong>Almacenamiento seguro</strong>
          <p>
            La API Key se cifra con DPAPI CurrentUser y solo se conserva en el
            backend.
          </p>
          <small>
            Esta primera etapa no realiza consultas, búsquedas ni descargas de
            imágenes.
          </small>
        </aside>
      </div>
      {deleteConfirm && (
        <SteamGridDbDeleteDialog
          onCancel={onCancelDelete}
          onConfirm={onConfirmDelete}
        />
      )}
    </>
  );
}

function RapidApiReviewsView({
  configuration,
  apiKeyDraft,
  editing,
  saving,
  errorMessage,
  deleteConfirm,
  onApiKeyDraftChange,
  onOpenApiKey,
  onSaveApiKey,
  onCancelEdit,
  onOpenDelete,
  onCancelDelete,
  onConfirmDelete,
}: {
  configuration: RapidApiReviewsConfigurationStatus | null;
  apiKeyDraft: string;
  editing: boolean;
  saving: boolean;
  errorMessage: string | null;
  deleteConfirm: boolean;
  onApiKeyDraftChange: (value: string) => void;
  onOpenApiKey: () => void;
  onSaveApiKey: () => void;
  onCancelEdit: () => void;
  onOpenDelete: () => void;
  onCancelDelete: () => void;
  onConfirmDelete: () => void;
}) {
  const configured = configuration?.apiKeyConfigured ?? false;
  return (
    <>
      <SettingsHeading
        eyebrow="Configuración · Integraciones · Reviews"
        title="OpenCritic / Metacritic"
        description="Configura la credencial compartida para consultar las puntuaciones de críticos."
      />
      <div className="steam-settings-layout">
        <div className="steam-settings-main">
          <article className="settings-panel">
            <div className="steam-account-heading">
              <ProviderIcon providerId="rapidapi-reviews" fallback="R" />
              <div>
                <p className="eyebrow">Proveedor de puntuaciones</p>
                <h2>OpenCritic / Metacritic</h2>
              </div>
              <strong
                className={`steam-config-status status-${rapidApiReviewsStatusLabel(configuration?.status)}`}
              >
                {rapidApiReviewsStatusLabel(configuration?.status)}
              </strong>
            </div>
            <div className="steam-summary-row">
              <span>Servicios</span>
              <strong>OpenCritic y Metacritic</strong>
            </div>
            <div className="steam-summary-row">
              <span>API Key</span>
              <strong>{configuration?.apiKeyMasked ?? "No configurada"}</strong>
            </div>
          </article>
          {editing || !configured ? (
            <article className="settings-panel settings-editor-panel">
              <p className="eyebrow">RapidAPI Key</p>
              <p className="settings-helper">
                Se guarda cifrada con DPAPI de Windows y nunca se envía al
                frontend. Esta misma clave se utiliza para OpenCritic y
                Metacritic.
              </p>
              <GamepadTextInput
                focusId="rapidapi-reviews-api-key-input"
                scopeId="settings-shell"
                value={apiKeyDraft}
                onChange={onApiKeyDraftChange}
                placeholder="Ingresa tu API Key de RapidAPI"
                ariaLabel="RapidAPI Key para OpenCritic y Metacritic"
                secure
                maxLength={256}
                className="settings-input"
              />
              <div className="settings-action-row">
                <Focusable
                  focusId="rapidapi-reviews-api-key-save"
                  scopeId="settings-shell"
                  className="settings-button primary"
                  disabled={saving}
                  onConfirm={onSaveApiKey}
                >
                  {saving ? "Guardando…" : "Guardar API Key"}
                </Focusable>
                {editing && (
                  <Focusable
                    focusId="rapidapi-reviews-api-key-cancel"
                    scopeId="settings-shell"
                    className="settings-button secondary"
                    onConfirm={onCancelEdit}
                  >
                    Cancelar
                  </Focusable>
                )}
              </div>
            </article>
          ) : (
            <Focusable
              focusId="rapidapi-reviews-change-key"
              scopeId="settings-shell"
              className="settings-action settings-action-primary"
              onConfirm={onOpenApiKey}
            >
              Cambiar API Key
            </Focusable>
          )}
          {configured && (
            <Focusable
              focusId="rapidapi-reviews-delete-key"
              scopeId="settings-shell"
              className="settings-action settings-action-danger"
              onConfirm={onOpenDelete}
            >
              Eliminar API Key
            </Focusable>
          )}
          {errorMessage && (
            <p className="settings-feedback is-error" role="alert">
              {errorMessage}
            </p>
          )}
        </div>
        <aside className="settings-panel settings-security-note">
          <strong>Almacenamiento seguro</strong>
          <p>
            La credencial se protege con DPAPI CurrentUser y solo permanece en
            el backend. La interfaz recibe únicamente su estado y máscara.
          </p>
          <small>
            El backend envía los headers X-RapidAPI-Key y X-RapidAPI-Host al
            proveedor correspondiente.
          </small>
        </aside>
      </div>
      {deleteConfirm && (
        <RapidApiReviewsDeleteDialog
          onCancel={onCancelDelete}
          onConfirm={onConfirmDelete}
        />
      )}
    </>
  );
}

function AIServicesView({
  configuration,
  apiKeyDraft,
  modelDraft,
  connectionStatus,
  saving,
  testing,
  errorMessage,
  onApiKeyDraftChange,
  onModelDraftChange,
  onSave,
  onTestConnection,
}: {
  configuration: AIConfigurationStatus | null;
  apiKeyDraft: string;
  modelDraft: string;
  connectionStatus: AIConnectionStatus | null;
  saving: boolean;
  testing: boolean;
  errorMessage: string | null;
  onApiKeyDraftChange: (value: string) => void;
  onModelDraftChange: (value: string) => void;
  onSave: () => void;
  onTestConnection: () => void;
}) {
  const providerFocus = useFocusable<HTMLSelectElement>({
    focusId: "ai-provider-select",
    scopeId: "settings-shell",
    onConfirm: () => undefined,
  });
  const status = connectionStatus ??
    configuration?.connection ?? { state: "not-configured" as const };
  const configured = configuration?.apiKeyConfigured ?? false;

  return (
    <>
      <SettingsHeading
        eyebrow="Configuración · Integraciones · Servicios IA"
        title="Servicios IA"
        description="Los Servicios IA permiten habilitar funciones avanzadas como consenso de reseñas, resúmenes de noticias, recomendaciones y futuras capacidades inteligentes de LumaDeck."
      />
      <div className="steam-settings-layout">
        <div className="steam-settings-main">
          <article className="settings-panel ai-settings-panel">
            <div className="steam-account-heading">
              <span className="provider-icon" aria-hidden="true">
                ✦
              </span>
              <div>
                <p className="eyebrow">Capacidad transversal</p>
                <h2>Configuración de Servicios IA</h2>
              </div>
              <strong
                className={`steam-config-status ai-status-${status.state}`}
              >
                {aiConnectionStatusLabel(status.state)}
              </strong>
            </div>
            <label className="ai-field-label" htmlFor="ai-provider-select">
              Proveedor
            </label>
            <select
              ref={providerFocus.ref}
              id="ai-provider-select"
              className="settings-input ai-provider-select"
              value="openrouter"
              aria-label="Proveedor de Servicios IA"
              data-focusable="true"
              data-focus-id="ai-provider-select"
              data-active={providerFocus.isActive ? "true" : "false"}
              tabIndex={providerFocus.tabIndex}
              onMouseEnter={providerFocus.onMouseEnter}
              onClick={providerFocus.onClick}
              onChange={() => undefined}
            >
              <option value="openrouter">OpenRouter</option>
              <option value="ollama" disabled>
                Ollama (Próximamente)
              </option>
              <option value="lm-studio" disabled>
                LM Studio (Próximamente)
              </option>
              <option value="openai" disabled>
                OpenAI (Próximamente)
              </option>
              <option value="gemini" disabled>
                Gemini (Próximamente)
              </option>
              <option value="claude" disabled>
                Claude (Próximamente)
              </option>
            </select>
            <div className="steam-summary-row">
              <span>Estado</span>
              <strong>{aiConnectionStatusLabel(status.state)}</strong>
            </div>
            <div className="steam-summary-row">
              <span>API Key</span>
              <strong>{configuration?.apiKeyMasked ?? "No configurada"}</strong>
            </div>
          </article>

          <article className="settings-panel settings-editor-panel">
            <p className="eyebrow">OpenRouter</p>
            <p className="settings-helper">
              La API Key se protege con DPAPI y nunca se devuelve al frontend.
              {configured
                ? " Déjala vacía para conservar la clave actual."
                : " Configúrala para habilitar la prueba de conexión."}
            </p>
            <label className="ai-field-label" htmlFor="ai-api-key-input">
              API Key
            </label>
            <GamepadTextInput
              focusId="ai-api-key-input"
              scopeId="settings-shell"
              value={apiKeyDraft}
              onChange={onApiKeyDraftChange}
              placeholder={
                configured
                  ? "Conservar API Key actual"
                  : "Ingresa tu API Key de OpenRouter"
              }
              ariaLabel="API Key de OpenRouter"
              secure
              maxLength={256}
              className="settings-input"
            />
            <label className="ai-field-label" htmlFor="ai-model-input">
              Modelo
            </label>
            <GamepadTextInput
              focusId="ai-model-input"
              scopeId="settings-shell"
              value={modelDraft}
              onChange={onModelDraftChange}
              placeholder="google/gemini-2.5-flash"
              ariaLabel="Modelo de OpenRouter"
              maxLength={256}
              className="settings-input"
            />
            <div className="settings-action-row">
              <Focusable
                focusId="ai-save"
                scopeId="settings-shell"
                className="settings-button primary"
                disabled={saving || testing}
                onConfirm={onSave}
              >
                {saving ? "Guardando…" : "Guardar configuración"}
              </Focusable>
              <Focusable
                focusId="ai-test-connection"
                scopeId="settings-shell"
                className="settings-button secondary"
                disabled={saving || testing}
                onConfirm={onTestConnection}
              >
                {testing ? "Conectando…" : "Probar conexión"}
              </Focusable>
            </div>
            {status.message && (
              <p
                className={`settings-feedback ${status.state === "connected" ? "is-success" : status.state === "not-configured" || status.state === "configured" ? "" : "is-error"}`}
                role="status"
              >
                {status.message}
              </p>
            )}
            {errorMessage && (
              <p className="settings-feedback is-error" role="alert">
                {errorMessage}
              </p>
            )}
          </article>
        </div>
        <aside className="settings-panel settings-security-note">
          <strong>Almacenamiento seguro</strong>
          <p>
            La API Key se guarda cifrada con DPAPI CurrentUser dentro de{" "}
            <code>provider_credentials</code>.
          </p>
          <small>
            La prueba de conexión valida OpenRouter desde Rust y no genera
            contenido, chat ni streaming.
          </small>
        </aside>
      </div>
    </>
  );
}

function aiConnectionStatusLabel(status: AIConnectionStatus["state"]): string {
  if (status === "connecting") return "Conectando";
  if (status === "connected") return "Conectado";
  if (status === "authentication-error") return "Error de autenticación";
  if (status === "offline") return "Sin conexión";
  if (status === "timeout") return "Timeout";
  if (status === "invalid-model") return "Modelo no válido";
  if (status === "credential-unavailable") return "Credencial no disponible";
  if (status === "error") return "Error";
  if (status === "configured") return "Configurado";
  return "No configurado";
}

function rapidApiReviewsStatusLabel(
  status: RapidApiReviewsConfigurationStatus["status"] | undefined,
): string {
  if (status === "configured") return "Configurada";
  if (status === "credential-unavailable") return "Credencial no disponible";
  return "No configurada";
}

function RapidApiReviewsDeleteDialog({
  onCancel,
  onConfirm,
}: {
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return (
    <div className="settings-modal-backdrop">
      <NavigationDialog
        scopeId="rapidapi-reviews-confirm-dialog"
        initialFocusId="rapidapi-reviews-delete-cancel"
        className="settings-modal"
        onBack={onCancel}
      >
        <p className="eyebrow">Confirmar eliminación</p>
        <h2>¿Eliminar API Key?</h2>
        <p>
          Se eliminará la credencial compartida para OpenCritic y Metacritic.
        </p>
        <div className="settings-action-row">
          <Focusable
            focusId="rapidapi-reviews-delete-confirm"
            scopeId="rapidapi-reviews-confirm-dialog"
            className="settings-button danger"
            onConfirm={onConfirm}
          >
            Eliminar
          </Focusable>
          <Focusable
            focusId="rapidapi-reviews-delete-cancel"
            scopeId="rapidapi-reviews-confirm-dialog"
            className="settings-button secondary"
            onConfirm={onCancel}
          >
            Cancelar
          </Focusable>
        </div>
      </NavigationDialog>
    </div>
  );
}

function steamGridDbStatusLabel(
  status: SteamGridDbConfigurationStatus["status"] | undefined,
): string {
  if (status === "configured") return "Configurada";
  if (status === "credential-unavailable") return "Credencial no disponible";
  return "No configurada";
}

function SteamGridDbDeleteDialog({
  onCancel,
  onConfirm,
}: {
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return (
    <div className="settings-modal-backdrop">
      <NavigationDialog
        scopeId="steamgriddb-confirm-dialog"
        initialFocusId="steamgriddb-delete-cancel"
        className="settings-modal"
        onBack={onCancel}
      >
        <p className="eyebrow">Confirmar eliminación</p>
        <h2>¿Eliminar API Key?</h2>
        <p>
          Solo se eliminará la credencial cifrada. Las preferencias futuras del
          proveedor se conservarán.
        </p>
        <div className="settings-action-row">
          <Focusable
            focusId="steamgriddb-delete-cancel"
            scopeId="steamgriddb-confirm-dialog"
            className="settings-button secondary"
            onConfirm={onCancel}
          >
            Cancelar
          </Focusable>
          <Focusable
            focusId="steamgriddb-delete-confirm"
            scopeId="steamgriddb-confirm-dialog"
            className="settings-button danger"
            onConfirm={onConfirm}
          >
            Eliminar
          </Focusable>
        </div>
      </NavigationDialog>
    </div>
  );
}

function HltbView({
  settings,
  status,
  errorMessage,
  syncStarting,
  onToggle,
  onStartSync,
  onCancelSync,
}: {
  settings: HltbSettings | null;
  status: HltbSyncStatus | null;
  errorMessage: string | null;
  syncStarting: boolean;
  onToggle: (key: keyof HltbSettings, value: boolean) => void;
  onStartSync: (onlyMissing: boolean) => void;
  onCancelSync: () => void;
}) {
  const syncing = syncStarting || status?.status === "running";
  const progress =
    status && status.totalCount > 0
      ? Math.round((status.processedCount / status.totalCount) * 100)
      : 0;
  const toggle = (key: keyof HltbSettings, label: string) => (
    <Focusable
      focusId={`hltb-${key}`}
      scopeId="settings-shell"
      className={`settings-toggle ${settings?.[key] ? "is-on" : ""}`}
      disabled={!settings}
      ariaSelected={settings?.[key] ?? false}
      onConfirm={() => onToggle(key, !(settings?.[key] ?? false))}
    >
      <span className="settings-toggle-box" aria-hidden="true">
        {settings?.[key] ? "✓" : ""}
      </span>
      {label}
    </Focusable>
  );

  return (
    <>
      <SettingsHeading
        eyebrow="Configuración · Integraciones · HowLongToBeat"
        title="HowLongToBeat"
        description="Duraciones estimadas para historia principal, extras y completista."
      />
      <div className="hltb-settings-layout">
        <div className="hltb-settings-main">
          <article className="settings-panel hltb-identity-panel">
            <div className="hltb-identity-heading">
              <span className="hltb-logo" aria-hidden="true">
                H
              </span>
              <div>
                <p className="eyebrow">Proveedor de metadatos</p>
                <h2>HowLongToBeat</h2>
                <p className="settings-helper">
                  No requiere cuenta, contraseña ni credenciales locales.
                </p>
              </div>
              <span
                className={`hltb-status is-${settings?.enabled ? "active" : "disabled"}`}
              >
                {settings?.enabled ? "Activo" : "Desactivado"}
              </span>
            </div>
            <div className="hltb-source-row">
              <span>Fuente</span>
              <strong>HowLongToBeat Community Database</strong>
            </div>
            <div className="hltb-source-row">
              <span>Última sincronización</span>
              <strong>{formatSyncTimestamp(status?.completedAt)}</strong>
            </div>
            <div className="hltb-source-row">
              <span>Resultado</span>
              <strong>{hltbStatusLabel(status?.status ?? "idle")}</strong>
            </div>
          </article>

          <article className="settings-panel hltb-sync-panel">
            <div className="steam-sync-heading">
              <div>
                <p className="eyebrow">Sincronización local</p>
                <h2>Actualizar duraciones</h2>
              </div>
              <span
                className={`steam-sync-status is-${status?.status ?? "idle"}`}
              >
                {hltbStatusLabel(status?.status ?? "idle")}
              </span>
            </div>
            <p className="settings-helper">
              Consulta juegos de tu biblioteca local. Puedes revisar solo los
              faltantes para evitar repetir consultas ya completadas; las
              duraciones anteriores se conservan si la fuente falla.
            </p>
            {syncing && (
              <div className="steam-sync-progress" aria-live="polite">
                <div className="steam-sync-progress-label">
                  <span>
                    {status?.processedCount ?? 0} / {status?.totalCount || "…"}{" "}
                    juegos
                  </span>
                  <strong>{progress}%</strong>
                </div>
                <div
                  className="steam-sync-progress-track"
                  role="progressbar"
                  aria-valuenow={progress}
                  aria-valuemin={0}
                  aria-valuemax={100}
                >
                  <span style={{ width: `${progress}%` }} />
                </div>
              </div>
            )}
            {!syncing && status && status.status !== "idle" && (
              <div className="hltb-stat-grid">
                <span>
                  Procesados <strong>{status.processedCount}</strong>
                </span>
                <span>
                  Encontrados <strong>{status.foundCount}</strong>
                </span>
                <span>
                  Sin coincidencia <strong>{status.unmatchedCount}</strong>
                </span>
                <span>
                  Exactas <strong>{status.exactMatchCount}</strong>
                </span>
                <span>
                  Aproximadas <strong>{status.approximateMatchCount}</strong>
                </span>
                <span>
                  Errores <strong>{status.errorCount}</strong>
                </span>
                <span>
                  Duración{" "}
                  <strong>{formatSyncDuration(status.durationMs)}</strong>
                </span>
              </div>
            )}
            {errorMessage && (
              <p className="settings-feedback is-error" role="alert">
                {errorMessage}
              </p>
            )}
            <div className="settings-action-row">
              <Focusable
                focusId="hltb-sync-missing"
                scopeId="settings-shell"
                className="settings-button primary"
                disabled={!settings?.enabled || syncing}
                onConfirm={() => onStartSync(true)}
              >
                {syncing
                  ? "Sincronizando…"
                  : `Consultar faltantes (${status?.unmatchedCount ?? 0})`}
              </Focusable>
              <Focusable
                focusId="hltb-sync-all"
                scopeId="settings-shell"
                className="settings-button secondary"
                disabled={!settings?.enabled || syncing}
                onConfirm={() => onStartSync(false)}
              >
                Sincronizar toda la biblioteca
              </Focusable>
              {syncing && (
                <Focusable
                  focusId="hltb-cancel"
                  scopeId="settings-shell"
                  className="settings-button secondary"
                  onConfirm={onCancelSync}
                >
                  Cancelar
                </Focusable>
              )}
            </div>
          </article>
          <HltbPendingMatchesPanel syncCompletedAt={status?.completedAt} />
        </div>
        <aside className="settings-panel hltb-options-panel">
          <p className="eyebrow">Opciones de integración</p>
          {toggle("enabled", "Obtener duraciones automáticamente")}
          {toggle(
            "syncWithSteam",
            "Actualizar durante la sincronización de Steam",
          )}
          {toggle("showMainStory", "Mostrar Main Story")}
          {toggle("showMainExtra", "Mostrar Main + Extras")}
          {toggle("showCompletionist", "Mostrar Completionist")}
        </aside>
      </div>
    </>
  );
}

interface SteamViewProps {
  configuration: SteamConfigurationStatus | null;
  configurationState: SteamConfigurationState;
  profile: SteamProfile | null;
  profileState: SteamProfileState;
  profileError: string | null;
  steamIdDraft: string;
  apiKeyDraft: string;
  editingSteamId: boolean;
  editingApiKey: boolean;
  errorMessage: string | null;
  confirmDisconnect: boolean;
  syncStatus: SteamSyncStatus | null;
  syncScope: SteamLibrarySyncScope;
  syncScopeSaving: boolean;
  syncStarting: boolean;
  achievementSyncStatus: SteamAchievementSyncStatus | null;
  achievementSyncStarting: boolean;
  imageSyncStatus: SteamImageSyncStatus | null;
  imageSyncStarting: boolean;
  onSteamIdDraftChange: (value: string) => void;
  onApiKeyDraftChange: (value: string) => void;
  onOpenSteamId: () => void;
  onOpenApiKey: () => void;
  onSaveSteamId: () => void;
  onSaveApiKey: () => void;
  onCancelEdit: () => boolean | void;
  onOpenDisconnect: () => void;
  onCancelDisconnect: () => void;
  onConfirmDisconnect: () => void;
  onStartSync: () => void;
  onSetSyncScope: (scope: SteamLibrarySyncScope) => void;
  onCancelSync: () => void;
  onStartAchievementSync: () => void;
  onStartImageSync: () => void;
  onCancelImageSync: () => void;
}

function HltbPendingMatchesPanel({
  syncCompletedAt,
}: {
  syncCompletedAt?: string;
}) {
  const [pending, setPending] = useState<HltbPendingMatch[]>([]);
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [candidates, setCandidates] = useState<Record<string, HltbCandidate[]>>(
    {},
  );
  const [busyGameId, setBusyGameId] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const matches = await providerSettingsService.getHltbPendingMatches();
      setPending(matches);
      setDrafts((current) => {
        const next = { ...current };
        for (const match of matches) {
          next[match.gameId] =
            current[match.gameId] ?? match.aliasQuery ?? match.title;
        }
        return next;
      });
    } catch {
      setErrorMessage("No se pudieron cargar las coincidencias pendientes.");
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh, syncCompletedAt]);

  const search = async (match: HltbPendingMatch) => {
    const query = drafts[match.gameId]?.trim() || match.title;
    setBusyGameId(match.gameId);
    setErrorMessage(null);
    try {
      const result = await providerSettingsService.searchHltbCandidates(query);
      setCandidates((current) => ({ ...current, [match.gameId]: result }));
    } catch {
      setErrorMessage(`No se pudo consultar HLTB para “${query}”.`);
    } finally {
      setBusyGameId(null);
    }
  };

  const save = async (match: HltbPendingMatch, candidate: HltbCandidate) => {
    const query = drafts[match.gameId]?.trim() || match.title;
    setBusyGameId(match.gameId);
    setErrorMessage(null);
    try {
      await providerSettingsService.setHltbMatchOverride(
        match.gameId,
        query,
        candidate,
      );
      setCandidates((current) => {
        const next = { ...current };
        delete next[match.gameId];
        return next;
      });
      await refresh();
    } catch {
      setErrorMessage(
        `No se pudo guardar la coincidencia para “${match.title}”.`,
      );
    } finally {
      setBusyGameId(null);
    }
  };

  const ignore = async (match: HltbPendingMatch) => {
    const query = drafts[match.gameId]?.trim() || match.title;
    setBusyGameId(match.gameId);
    setErrorMessage(null);
    try {
      await providerSettingsService.ignoreHltbMatch(match.gameId, query);
      await refresh();
    } catch {
      setErrorMessage(`No se pudo ignorar “${match.title}”.`);
    } finally {
      setBusyGameId(null);
    }
  };

  return (
    <article className="settings-panel hltb-review-panel">
      <div className="hltb-review-heading">
        <div>
          <p className="eyebrow">Resolución manual</p>
          <h2>Coincidencias pendientes</h2>
        </div>
        <strong>{pending.length}</strong>
      </div>
      <p className="settings-helper">
        Define un alias, consulta HLTB y selecciona el resultado correcto. El
        título original de Steam no se modifica.
      </p>
      {errorMessage && (
        <p className="settings-feedback is-error" role="alert">
          {errorMessage}
        </p>
      )}
      {pending.length === 0 ? (
        <p className="hltb-review-empty">
          No hay juegos pendientes de revisar.
        </p>
      ) : (
        <div className="hltb-review-list">
          {pending.map((match) => {
            const query =
              drafts[match.gameId] ?? match.aliasQuery ?? match.title;
            const matchCandidates = candidates[match.gameId] ?? [];
            const busy = busyGameId === match.gameId;
            return (
              <div className="hltb-review-item" key={match.gameId}>
                <div className="hltb-review-item-heading">
                  <strong>{match.title}</strong>
                  <span>{match.gameId.replace(/^steam-/, "Steam ")}</span>
                </div>
                <div className="hltb-review-controls">
                  <GamepadTextInput
                    focusId={`hltb-query-${match.gameId}`}
                    scopeId="settings-shell"
                    className="settings-input hltb-review-input"
                    ariaLabel={`Alias HLTB para ${match.title}`}
                    value={query}
                    onChange={(value) =>
                      setDrafts((current) => ({
                        ...current,
                        [match.gameId]: value,
                      }))
                    }
                  />
                  <Focusable
                    focusId={`hltb-search-${match.gameId}`}
                    scopeId="settings-shell"
                    className="settings-button secondary"
                    disabled={busy}
                    onConfirm={() => void search(match)}
                  >
                    {busy ? "Consultando…" : "Buscar"}
                  </Focusable>
                  <Focusable
                    focusId={`hltb-ignore-${match.gameId}`}
                    scopeId="settings-shell"
                    className="settings-button secondary"
                    disabled={busy}
                    onConfirm={() => void ignore(match)}
                  >
                    Ignorar
                  </Focusable>
                </div>
                {matchCandidates.length > 0 && (
                  <div className="hltb-candidate-list">
                    {matchCandidates.map((candidate) => (
                      <Focusable
                        key={candidate.hltbId}
                        focusId={`hltb-assign-${match.gameId}-${candidate.hltbId}`}
                        scopeId="settings-shell"
                        className="hltb-candidate"
                        disabled={busy}
                        onConfirm={() => void save(match, candidate)}
                      >
                        <strong>{candidate.title}</strong>
                        <span>
                          Historia{" "}
                          {formatHltbMinutes(candidate.mainStoryMinutes)}
                          {" · "}
                          Completista{" "}
                          {formatHltbMinutes(candidate.completionistMinutes)}
                        </span>
                      </Focusable>
                    ))}
                  </div>
                )}
                {matchCandidates.length === 0 && !busy && (
                  <span className="hltb-review-hint">
                    Busca para mostrar candidatos de HLTB.
                  </span>
                )}
              </div>
            );
          })}
        </div>
      )}
    </article>
  );
}

function formatHltbMinutes(minutes?: number): string {
  if (!minutes || minutes <= 0) return "—";
  return `${Math.max(1, Math.round(minutes / 60))} h`;
}

function SteamView(props: SteamViewProps) {
  const statusLabel =
    props.configurationState === "loading"
      ? "Cargando…"
      : props.profileState === "loaded"
        ? "Cuenta verificada"
        : props.profileState === "loading" &&
            props.configuration?.status === "configured"
          ? "Verificando…"
          : props.profileState === "offline"
            ? "Sin conexión"
            : props.profileState === "error"
              ? "Error de Steam"
              : statusToLabel(props.configuration?.status ?? "not-configured");
  const isSaving = props.configurationState === "saving";
  const isSyncing =
    props.syncStarting || props.syncStatus?.status === "running";
  const isImageSyncing =
    props.imageSyncStarting || props.imageSyncStatus?.status === "running";
  const isAchievementSyncing =
    props.achievementSyncStarting ||
    props.achievementSyncStatus?.status === "running";
  const progressTotal = props.syncStatus?.progressTotal ?? 0;
  const progressCompleted = props.syncStatus?.progressCompleted ?? 0;
  const progressPercent =
    progressTotal > 0
      ? Math.round((progressCompleted / progressTotal) * 100)
      : 0;
  return (
    <>
      <SettingsHeading
        eyebrow="Configuración · Integraciones · Steam"
        title="Steam"
        description="Administra tu cuenta de Steam"
      />
      <div className="steam-settings-layout">
        <div className="steam-settings-main">
          <article className="settings-panel steam-account-panel">
            <div className="steam-account-heading">
              <ProviderIcon providerId="steam" fallback="S" />
              <div>
                <p className="eyebrow">Cuenta</p>
                <h2>Steam</h2>
              </div>
              <span
                className={`steam-config-status status-${props.configuration?.status ?? "not-configured"}`}
              >
                {statusLabel}
              </span>
            </div>
            {props.profile && (
              <div className="steam-profile-summary">
                <img
                  className="steam-profile-avatar"
                  src={props.profile.avatarUrl}
                  alt={`Avatar de ${props.profile.personaName}`}
                />
                <div>
                  <p className="eyebrow">Perfil</p>
                  <strong>{props.profile.personaName}</strong>
                </div>
              </div>
            )}
            <div className="steam-summary-row">
              <span>SteamID64</span>
              <strong>
                {props.profile?.steamId64 ??
                  props.configuration?.steamId64Masked ??
                  "No configurado"}
              </strong>
            </div>
            <div className="steam-summary-row">
              <span>Nombre</span>
              <strong>{props.profile?.personaName ?? "Consultando…"}</strong>
            </div>
            <div className="steam-summary-row">
              <span>País</span>
              <strong>{props.profile?.countryCode ?? "No disponible"}</strong>
            </div>
            <div className="steam-summary-row">
              <span>Juegos</span>
              <strong>
                {props.profile ? props.profile.gameCount : "Consultando…"}
              </strong>
            </div>
            <div className="steam-summary-row">
              <span>API Key</span>
              <strong>
                {props.configuration?.apiKeyMasked ?? "No configurada"}
              </strong>
            </div>
          </article>
          <article className="settings-panel steam-sync-panel">
            <div className="steam-sync-heading">
              <div>
                <p className="eyebrow">Biblioteca Steam</p>
                <h2>Sincronizar biblioteca</h2>
              </div>
              <span
                className={`steam-sync-status is-${props.syncStatus?.status ?? "idle"}`}
              >
                {syncStatusLabel(props.syncStatus?.status ?? "idle")}
              </span>
            </div>
            <p className="settings-helper">
              Importa juegos y actualiza únicamente los datos propiedad de
              Steam.
            </p>
            <div
              className="settings-choice-group"
              aria-label="Alcance de sincronización"
            >
              <span className="eyebrow">Alcance</span>
              <div className="settings-action-row">
                <Focusable
                  focusId="steam-sync-scope-all"
                  scopeId="settings-shell"
                  className={`settings-button ${props.syncScope === "all" ? "primary" : "secondary"}`}
                  disabled={isSyncing || props.syncScopeSaving}
                  ariaSelected={props.syncScope === "all"}
                  onConfirm={() => props.onSetSyncScope("all")}
                >
                  Biblioteca completa
                </Focusable>
                <Focusable
                  focusId="steam-sync-scope-installed"
                  scopeId="settings-shell"
                  className={`settings-button ${props.syncScope === "installed" ? "primary" : "secondary"}`}
                  disabled={isSyncing || props.syncScopeSaving}
                  ariaSelected={props.syncScope === "installed"}
                  onConfirm={() => props.onSetSyncScope("installed")}
                >
                  Solo juegos instalados
                </Focusable>
              </div>
              <p className="settings-helper">
                {props.syncScope === "installed"
                  ? "Solo consulta los AppID que Steam detecta como instalados. Los demás juegos guardados se conservan y vuelven a aparecer al elegir Biblioteca completa."
                  : "Consulta todos los juegos de tu biblioteca de Steam y conserva los juegos ya guardados, aunque no estén instalados."}
              </p>
            </div>
            {isSyncing && (
              <div className="steam-sync-progress" aria-live="polite">
                <div className="steam-sync-progress-label">
                  <span>
                    {progressCompleted} / {progressTotal || "…"} juegos
                  </span>
                  <strong>{progressPercent}%</strong>
                </div>
                <div
                  className="steam-sync-progress-track"
                  role="progressbar"
                  aria-valuenow={progressPercent}
                  aria-valuemin={0}
                  aria-valuemax={100}
                >
                  <span style={{ width: `${progressPercent}%` }} />
                </div>
              </div>
            )}
            {!isSyncing &&
              props.syncStatus &&
              props.syncStatus.status !== "idle" && (
                <div className="steam-sync-summary">
                  <span>
                    Encontrados <strong>{props.syncStatus.foundCount}</strong>
                  </span>
                  <span>
                    Creados <strong>{props.syncStatus.createdCount}</strong>
                  </span>
                  <span>
                    Actualizados{" "}
                    <strong>{props.syncStatus.updatedCount}</strong>
                  </span>
                  <span>
                    Duración{" "}
                    <strong>
                      {formatSyncDuration(props.syncStatus.durationMs)}
                    </strong>
                  </span>
                  <span>
                    Última sincronización{" "}
                    <strong>
                      {formatSyncTimestamp(props.syncStatus.completedAt)}
                    </strong>
                  </span>
                </div>
              )}
            <div className="settings-action-row">
              <Focusable
                focusId="steam-sync-library"
                scopeId="settings-shell"
                navigationRegion={{
                  regionId: "settings-content",
                  parentRegionId: "main-navigation",
                  exitFocusId: "main-nav-settings",
                }}
                className="settings-button primary"
                disabled={
                  isSyncing || props.configuration?.status !== "configured"
                }
                onConfirm={props.onStartSync}
              >
                {isSyncing ? "Sincronizando…" : "Sincronizar biblioteca"}
              </Focusable>
              {isSyncing && (
                <Focusable
                  focusId="steam-sync-cancel"
                  scopeId="settings-shell"
                  navigationRegion={{
                    regionId: "settings-content",
                    parentRegionId: "main-navigation",
                    exitFocusId: "main-nav-settings",
                  }}
                  className="settings-button secondary"
                  onConfirm={props.onCancelSync}
                >
                  Cancelar después de la llamada actual
                </Focusable>
              )}
            </div>
          </article>
          <article className="settings-panel steam-sync-panel steam-achievements-panel">
            <div className="steam-sync-heading">
              <div>
                <p className="eyebrow">Progreso Steam</p>
                <h2>Actualizar trofeos</h2>
              </div>
              <span
                className={`steam-sync-status is-${props.achievementSyncStatus?.status ?? "idle"}`}
              >
                {syncStatusLabel(props.achievementSyncStatus?.status ?? "idle")}
              </span>
            </div>
            <p className="settings-helper">
              Consulta únicamente los trofeos de los juegos que ya están
              guardados en LumaDeck. No importa ni vuelve a sincronizar la
              biblioteca.
            </p>
            {!isAchievementSyncing &&
              props.achievementSyncStatus &&
              props.achievementSyncStatus.status !== "idle" && (
                <div className="steam-sync-summary">
                  <span>
                    Juegos{" "}
                    <strong>{props.achievementSyncStatus.foundCount}</strong>
                  </span>
                  <span>
                    Actualizados{" "}
                    <strong>{props.achievementSyncStatus.updatedCount}</strong>
                  </span>
                  <span>
                    Omitidos{" "}
                    <strong>{props.achievementSyncStatus.skippedCount}</strong>
                  </span>
                  <span>
                    Duración{" "}
                    <strong>
                      {formatSyncDuration(
                        props.achievementSyncStatus.durationMs,
                      )}
                    </strong>
                  </span>
                </div>
              )}
            <div className="settings-action-row">
              <Focusable
                focusId="steam-sync-achievements"
                scopeId="settings-shell"
                className="settings-button primary"
                disabled={
                  isAchievementSyncing ||
                  isSyncing ||
                  isImageSyncing ||
                  props.configuration?.status !== "configured"
                }
                onConfirm={props.onStartAchievementSync}
              >
                {isAchievementSyncing
                  ? "Actualizando trofeos…"
                  : "Actualizar trofeos"}
              </Focusable>
            </div>
          </article>
          <article className="settings-panel steam-sync-panel steam-images-panel">
            <div className="steam-sync-heading">
              <div>
                <p className="eyebrow">Assets locales Steam</p>
                <h2>Sincronizar imágenes</h2>
              </div>
              <span
                className={`steam-sync-status is-${props.imageSyncStatus?.status ?? "idle"}`}
              >
                {syncStatusLabel(props.imageSyncStatus?.status ?? "idle")}
              </span>
            </div>
            <p className="settings-helper">
              Descarga portada horizontal, vertical, logo, hero y screenshots en
              WebP a máxima resolución disponible.
            </p>
            {isImageSyncing && (
              <div className="steam-sync-progress" aria-live="polite">
                <div className="steam-sync-progress-label">
                  <span>
                    {props.imageSyncStatus?.progressCompleted ?? 0} /{" "}
                    {props.imageSyncStatus?.progressTotal || "…"} assets
                  </span>
                  <strong>{syncProgressPercent(props.imageSyncStatus)}%</strong>
                </div>
                <div
                  className="steam-sync-progress-track"
                  role="progressbar"
                  aria-valuenow={syncProgressPercent(props.imageSyncStatus)}
                  aria-valuemin={0}
                  aria-valuemax={100}
                >
                  <span
                    style={{
                      width: `${syncProgressPercent(props.imageSyncStatus)}%`,
                    }}
                  />
                </div>
              </div>
            )}
            {!isImageSyncing &&
              props.imageSyncStatus &&
              props.imageSyncStatus.status !== "idle" && (
                <div className="steam-sync-summary">
                  <span>
                    Assets <strong>{props.imageSyncStatus.foundCount}</strong>
                  </span>
                  <span>
                    Descargados{" "}
                    <strong>{props.imageSyncStatus.downloadedCount}</strong>
                  </span>
                  <span>
                    Omitidos{" "}
                    <strong>{props.imageSyncStatus.skippedCount}</strong>
                  </span>
                  <span>
                    Duración{" "}
                    <strong>
                      {formatSyncDuration(props.imageSyncStatus.durationMs)}
                    </strong>
                  </span>
                  <span>
                    Última sincronización{" "}
                    <strong>
                      {formatSyncTimestamp(props.imageSyncStatus.completedAt)}
                    </strong>
                  </span>
                </div>
              )}
            <div className="settings-action-row">
              <Focusable
                focusId="steam-sync-images"
                scopeId="settings-shell"
                className="settings-button primary"
                disabled={
                  isImageSyncing ||
                  isSyncing ||
                  props.configuration?.status !== "configured"
                }
                onConfirm={props.onStartImageSync}
              >
                {isImageSyncing
                  ? "Sincronizando imágenes…"
                  : "Sincronizar imágenes"}
              </Focusable>
              {isImageSyncing && (
                <Focusable
                  focusId="steam-sync-images-cancel"
                  scopeId="settings-shell"
                  className="settings-button secondary"
                  onConfirm={props.onCancelImageSync}
                >
                  Cancelar después de la llamada actual
                </Focusable>
              )}
            </div>
          </article>
          {props.profileError && (
            <p className="settings-feedback is-error" role="alert">
              {props.profileError}
            </p>
          )}
          {props.editingSteamId || !props.configuration?.accountId ? (
            <article className="settings-panel settings-editor-panel">
              <p className="eyebrow">SteamID64</p>
              <p className="settings-helper">
                Ingresa tu SteamID64 de 17 dígitos.
              </p>
              <GamepadTextInput
                focusId="steam-id-input"
                scopeId="settings-shell"
                value={props.steamIdDraft}
                onChange={props.onSteamIdDraftChange}
                placeholder="Ingresa tu SteamID64"
                ariaLabel="SteamID64"
                inputMode="numeric"
                maxLength={17}
                className="settings-input"
              />
              <div className="settings-action-row">
                <Focusable
                  focusId="steam-id-save"
                  scopeId="settings-shell"
                  className="settings-button primary"
                  disabled={isSaving}
                  onConfirm={props.onSaveSteamId}
                >
                  {isSaving ? "Guardando…" : "Guardar SteamID64"}
                </Focusable>
                {props.editingSteamId && (
                  <Focusable
                    focusId="steam-id-cancel"
                    scopeId="settings-shell"
                    className="settings-button secondary"
                    onConfirm={props.onCancelEdit}
                  >
                    Cancelar
                  </Focusable>
                )}
              </div>
            </article>
          ) : (
            <Focusable
              focusId="steam-change-id"
              scopeId="settings-shell"
              className="settings-action settings-action-primary"
              onConfirm={props.onOpenSteamId}
            >
              ✎ Cambiar SteamID64
            </Focusable>
          )}
          {props.editingApiKey || !props.configuration?.apiKeyConfigured ? (
            <article className="settings-panel settings-editor-panel">
              <p className="eyebrow">Steam Web API Key</p>
              <p className="settings-helper">
                Se guardará cifrada con DPAPI de Windows. Nunca se mostrará
                completa.
              </p>
              <GamepadTextInput
                focusId="steam-api-key-input"
                scopeId="settings-shell"
                value={props.apiKeyDraft}
                onChange={props.onApiKeyDraftChange}
                placeholder="Ingresa tu API Key"
                ariaLabel="Steam Web API Key"
                secure
                maxLength={64}
                className="settings-input"
              />
              <div className="settings-action-row">
                <Focusable
                  focusId="steam-api-save"
                  scopeId="settings-shell"
                  className="settings-button primary"
                  disabled={isSaving}
                  onConfirm={props.onSaveApiKey}
                >
                  {isSaving ? "Guardando…" : "Guardar API Key"}
                </Focusable>
                {props.editingApiKey && (
                  <Focusable
                    focusId="steam-api-cancel"
                    scopeId="settings-shell"
                    className="settings-button secondary"
                    onConfirm={props.onCancelEdit}
                  >
                    Cancelar
                  </Focusable>
                )}
              </div>
            </article>
          ) : (
            <Focusable
              focusId="steam-change-api"
              scopeId="settings-shell"
              className="settings-action settings-action-primary"
              onConfirm={props.onOpenApiKey}
            >
              ✎ Cambiar API Key
            </Focusable>
          )}
          {props.errorMessage && (
            <p className="settings-feedback is-error" role="alert">
              {props.errorMessage}
            </p>
          )}
          {props.configuration?.accountId && (
            <Focusable
              focusId="steam-disconnect"
              scopeId="settings-shell"
              className="settings-action settings-action-danger"
              onConfirm={props.onOpenDisconnect}
            >
              Desconectar cuenta
            </Focusable>
          )}
        </div>
        <aside className="settings-panel settings-security-note">
          <strong>Configuración local</strong>
          <p>
            La cuenta se guarda en SQLite. La API Key se cifra con DPAPI y solo
            se conserva en el backend de Windows.
          </p>
          <small>
            La sincronización de la biblioteca estará disponible en la siguiente
            fase.
          </small>
        </aside>
      </div>
      {props.confirmDisconnect && (
        <DisconnectDialog
          onCancel={props.onCancelDisconnect}
          onConfirm={props.onConfirmDisconnect}
        />
      )}
    </>
  );
}

function StorageView({
  status,
  loading,
  errorMessage,
  deleteSource,
  onToggleDeleteSource,
  onOpenMigration,
}: {
  status: StorageStatus | null;
  loading: boolean;
  errorMessage: string | null;
  deleteSource: boolean;
  onToggleDeleteSource: () => void;
  onOpenMigration: () => void;
}) {
  const migration = status?.migration;
  const isRunning = loading || migration?.status === "running";
  const progressPercent =
    migration && migration.totalFiles > 0
      ? Math.round((migration.filesCopied / migration.totalFiles) * 100)
      : 0;
  const targetMode = status?.mode === "portable" ? "appData" : "portable";
  const targetPath =
    targetMode === "portable" ? status?.portablePath : status?.normalPath;

  return (
    <>
      <SettingsHeading
        eyebrow="Configuración · Almacenamiento"
        title="Almacenamiento"
        description="Mueve tus datos entre AppData y el modo portable"
      />
      <div className="steam-settings-layout">
        <div className="steam-settings-main">
          <article className="settings-panel steam-account-panel">
            <div className="steam-sync-heading">
              <div>
                <p className="eyebrow">Modo actual</p>
                <h2>{status?.mode === "portable" ? "Portable" : "Sistema"}</h2>
              </div>
              <span className="steam-sync-status is-completed">
                {isRunning ? "Migrando…" : "Activo"}
              </span>
            </div>
            <div className="steam-summary-row">
              <span>Ruta actual</span>
              <strong>{status?.currentPath ?? "Cargando…"}</strong>
            </div>
            <div className="steam-summary-row">
              <span>Espacio usado</span>
              <strong>{formatStorageBytes(status?.usedBytes ?? 0)}</strong>
            </div>
            <div className="steam-summary-row">
              <span>Destino</span>
              <strong>{targetPath ?? "Cargando…"}</strong>
            </div>
          </article>

          <article className="settings-panel steam-sync-panel">
            <div className="steam-sync-heading">
              <div>
                <p className="eyebrow">Migración segura</p>
                <h2>
                  {targetMode === "portable"
                    ? "Convertir en portable"
                    : "Usar almacenamiento del sistema"}
                </h2>
              </div>
              <span
                className={`steam-sync-status is-${migration?.status ?? "idle"}`}
              >
                {storageMigrationStatusLabel(migration?.status ?? "idle")}
              </span>
            </div>
            <p className="settings-helper">
              Se copiarán la base de datos, biblioteca, imágenes, caché y logs.
              La nueva ubicación se activará al reiniciar LumaDeck.
            </p>
            {isRunning && migration && (
              <div className="steam-sync-progress" aria-live="polite">
                <div className="steam-sync-progress-label">
                  <span>
                    {migration.filesCopied} / {migration.totalFiles || "…"}{" "}
                    archivos · {formatStorageBytes(migration.bytesCopied)}
                  </span>
                  <strong>{progressPercent}%</strong>
                </div>
                <div
                  className="steam-sync-progress-track"
                  role="progressbar"
                  aria-valuenow={progressPercent}
                  aria-valuemin={0}
                  aria-valuemax={100}
                >
                  <span style={{ width: `${progressPercent}%` }} />
                </div>
              </div>
            )}
            {migration?.status === "completed" && migration.needsRestart && (
              <p className="settings-feedback" role="status">
                Migración completada. Reinicia LumaDeck para usar la nueva ruta.
              </p>
            )}
            {(errorMessage || migration?.errorMessage) && (
              <p className="settings-feedback is-error" role="alert">
                {errorMessage ?? migration?.errorMessage}
              </p>
            )}
            <div className="settings-action-row">
              <Focusable
                focusId="storage-migrate"
                scopeId="settings-shell"
                className="settings-button primary"
                disabled={!status || isRunning}
                onConfirm={onOpenMigration}
              >
                {isRunning
                  ? "Migrando…"
                  : targetMode === "portable"
                    ? "Convertir en portable"
                    : "Usar almacenamiento del sistema"}
              </Focusable>
              <Focusable
                focusId="storage-delete-source"
                scopeId="settings-shell"
                className="settings-button secondary"
                disabled={!status || isRunning}
                onConfirm={onToggleDeleteSource}
              >
                {deleteSource
                  ? "☑ Limpiar origen tras confirmar"
                  : "☐ Conservar origen como respaldo"}
              </Focusable>
            </div>
          </article>
        </div>
        <aside className="settings-panel settings-security-note">
          <strong>Credenciales DPAPI</strong>
          <p>
            La migración conserva la base cifrada. Si se abre en otro usuario o
            PC, la API Key puede requerir volver a ingresarse sin perder juegos
            ni imágenes.
          </p>
        </aside>
      </div>
    </>
  );
}

function StorageMigrationDialog({
  status,
  deleteSource,
  onCancel,
  onConfirm,
}: {
  status: StorageStatus;
  deleteSource: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const targetMode = status.mode === "portable" ? "appData" : "portable";
  const targetPath =
    targetMode === "portable" ? status.portablePath : status.normalPath;
  return (
    <div className="settings-modal-backdrop">
      <NavigationDialog
        scopeId="storage-confirm-dialog"
        initialFocusId="storage-migration-cancel"
        className="settings-modal"
        onBack={onCancel}
      >
        <p className="eyebrow">Confirmar migración</p>
        <h2>
          ¿
          {targetMode === "portable"
            ? "Convertir en portable"
            : "Usar almacenamiento del sistema"}
          ?
        </h2>
        <p>
          Se copiarán todos los datos a <strong>{targetPath}</strong> y se
          validará la base antes de activar el destino.
        </p>
        <p>
          {deleteSource
            ? "El origen se conservará temporalmente y se limpiará después del primer inicio correcto."
            : "El origen se conservará como respaldo."}
        </p>
        <div className="settings-action-row">
          <Focusable
            focusId="storage-migration-cancel"
            scopeId="storage-confirm-dialog"
            className="settings-button secondary"
            onConfirm={onCancel}
          >
            Cancelar
          </Focusable>
          <Focusable
            focusId="storage-migration-confirm"
            scopeId="storage-confirm-dialog"
            className="settings-button primary"
            onConfirm={onConfirm}
          >
            Confirmar y migrar
          </Focusable>
        </div>
      </NavigationDialog>
    </div>
  );
}

function DisconnectDialog({
  onCancel,
  onConfirm,
}: {
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return (
    <div className="settings-modal-backdrop">
      <NavigationDialog
        scopeId="settings-confirm-dialog"
        initialFocusId="disconnect-cancel"
        className="settings-modal"
        onBack={onCancel}
      >
        <p className="eyebrow">Confirmar desconexión</p>
        <h2>¿Desconectar Steam?</h2>
        <p>
          Se eliminarán el SteamID64 y la credencial cifrada. No se borrarán
          otros datos de LumaDeck.
        </p>
        <div className="settings-action-row">
          <Focusable
            focusId="disconnect-cancel"
            scopeId="settings-confirm-dialog"
            className="settings-button secondary"
            onConfirm={onCancel}
          >
            Cancelar
          </Focusable>
          <Focusable
            focusId="disconnect-confirm"
            scopeId="settings-confirm-dialog"
            className="settings-button danger"
            onConfirm={onConfirm}
          >
            Desconectar
          </Focusable>
        </div>
      </NavigationDialog>
    </div>
  );
}

function SettingsHeading({
  eyebrow,
  title,
  description,
}: {
  eyebrow: string;
  title: string;
  description: string;
}) {
  return (
    <div className="settings-heading">
      <div>
        <p className="eyebrow">{eyebrow}</p>
        <h1 id="settings-heading">{title}</h1>
        <p>{description}</p>
      </div>
      <span className="page-hint">A seleccionar · B atrás</span>
    </div>
  );
}

function statusToLabel(status: SteamConfigurationStatus["status"]): string {
  if (status === "configured") return "Configurado";
  if (status === "partially-configured") return "Parcial";
  if (status === "credential-unavailable")
    return "Credencial no disponible en este equipo";
  return "No configurado";
}

function syncStatusLabel(status: SteamSyncStatus["status"]): string {
  if (status === "running") return "En curso";
  if (status === "completed") return "Completada";
  if (status === "cancelled") return "Cancelada";
  if (status === "error") return "Error";
  return "Sin sincronizar";
}

function hltbStatusLabel(status: HltbSyncStatus["status"]): string {
  if (status === "running") return "Sincronizando";
  if (status === "completed") return "Completado";
  if (status === "cancelled") return "Cancelado";
  if (status === "error") return "Error parcial";
  return "Sin datos";
}

function formatSyncDuration(durationMs: number | undefined): string {
  if (durationMs === undefined) return "—";
  return durationMs < 1000
    ? `${durationMs} ms`
    : `${(durationMs / 1000).toFixed(1)} s`;
}

function formatSyncTimestamp(value: string | undefined): string {
  if (!value) return "—";
  const seconds = Number(value);
  if (!Number.isFinite(seconds)) return value;
  return new Date(seconds * 1000).toLocaleString();
}

function syncProgressPercent(status: SteamImageSyncStatus | null): number {
  const total = status?.progressTotal ?? 0;
  return total > 0
    ? Math.round(((status?.progressCompleted ?? 0) / total) * 100)
    : 0;
}

function storageMigrationStatusLabel(
  status: StorageStatus["migration"]["status"],
): string {
  if (status === "running") return "En curso";
  if (status === "completed") return "Completada";
  if (status === "error") return "Error";
  return "Lista";
}

function formatStorageBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024)
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function profileStateFromError(error: unknown): SteamProfileState {
  return error instanceof ProviderSettingsError &&
    error.code === "STEAM_OFFLINE"
    ? "offline"
    : "error";
}

function toSteamProfileErrorMessage(error: unknown): string {
  if (!(error instanceof ProviderSettingsError))
    return "No se pudo consultar el perfil de Steam.";
  if (error.code === "STEAM_OFFLINE")
    return "Steam no responde. Comprueba tu conexión e inténtalo de nuevo.";
  if (error.code === "STEAM_INVALID_RESPONSE")
    return "Steam devolvió datos no válidos.";
  if (error.code === "STEAM_API_ERROR")
    return "Steam rechazó la consulta del perfil.";
  if (error.code === "CREDENTIAL_ERROR")
    return "Credencial no disponible en este equipo. Ingresa nuevamente la API Key.";
  if (error.code === "VALIDATION_ERROR")
    return "La cuenta de Steam no está configurada completamente.";
  return "No se pudo consultar el perfil de Steam.";
}

function toSafeErrorMessage(error: unknown): string {
  if (!(error instanceof ProviderSettingsError))
    return "No se pudo guardar la configuración. Inténtalo de nuevo.";
  const message =
    error.code === "IPC_INVALID_ARGUMENTS"
      ? "No se pudo comunicar correctamente con LumaDeck."
      : error.code === "IPC_COMMAND_NOT_FOUND"
        ? "Esta función no está disponible en esta versión de LumaDeck."
        : error.code === "VALIDATION_ERROR"
          ? "Los datos de Steam no tienen un formato válido."
          : error.code === "CREDENTIAL_ERROR"
            ? "La credencial no se puede descifrar. Configura una API Key nueva."
            : error.code === "LAUNCHBOX_CATALOG_NOT_READY"
              ? "El catálogo de LaunchBox todavía se está preparando."
              : error.code === "LAUNCHBOX_DATABASE_LOCK"
                ? "LaunchBox está temporalmente ocupado. Intenta nuevamente."
                : error.code === "LAUNCHBOX_CATALOG_UNAVAILABLE"
                  ? "El catálogo de LaunchBox no está disponible."
                  : error.code === "LAUNCHBOX_UPDATE_FAILED_WITH_FALLBACK"
                    ? "No se pudo actualizar el catálogo. Se seguirá utilizando la versión anterior."
                    : error.code === "LAUNCHBOX_CATALOG_UPDATE_IN_PROGRESS"
                      ? "El catálogo de LaunchBox ya se está actualizando."
                      : error.code === "DATABASE_ERROR"
                        ? "No se pudo guardar la configuración local."
                        : "No se pudo guardar la configuración. Inténtalo de nuevo.";
  if (import.meta.env.DEV)
    return `${message} Código: ${error.code}${error.diagnostic ? ` (${error.diagnostic})` : ""}`;
  return message;
}

function toSyncErrorMessage(error: unknown): string {
  if (!(error instanceof ProviderSettingsError))
    return "No se pudo sincronizar la biblioteca.";
  if (error.code === "STEAM_OFFLINE")
    return "Steam no responde. Inténtalo de nuevo.";
  if (error.code === "STEAM_SYNC_ALREADY_RUNNING")
    return "Ya hay una sincronización en curso.";
  if (error.code === "STEAM_SYNC_CANCELLED")
    return "La sincronización fue cancelada.";
  if (error.code === "CREDENTIAL_ERROR")
    return "Credencial no disponible en este equipo. Vuelve a introducir la API Key de Steam.";
  if (error.code === "STEAM_INSTALLED_GAMES_UNAVAILABLE")
    return "No se encontró la instalación local de Steam para detectar juegos instalados.";
  if (error.code === "VALIDATION_ERROR")
    return "Configura el SteamID64 y la API Key antes de sincronizar.";
  return "No se pudo sincronizar la biblioteca de Steam.";
}

function toImageSyncErrorMessage(error: unknown): string {
  if (!(error instanceof ProviderSettingsError))
    return "No se pudieron sincronizar las imágenes de Steam.";
  if (error.code === "STEAM_OFFLINE")
    return "Steam no responde. Inténtalo de nuevo.";
  if (error.code === "STEAM_IMAGE_SYNC_ALREADY_RUNNING")
    return "Ya hay una sincronización de imágenes en curso.";
  if (error.code === "STEAM_IMAGE_SYNC_CANCELLED")
    return "La sincronización de imágenes fue cancelada.";
  if (error.code === "VALIDATION_ERROR")
    return "Configura el SteamID64 y la API Key antes de sincronizar.";
  return "No se pudieron sincronizar las imágenes de Steam.";
}

function toHltbErrorMessage(error: unknown): string {
  if (!(error instanceof ProviderSettingsError))
    return "No se pudo sincronizar HowLongToBeat.";
  if (error.code === "HLTB_SYNC_ALREADY_RUNNING")
    return "Ya hay una sincronización de HLTB en curso.";
  if (error.code === "HLTB_SYNC_CANCELLED")
    return "La sincronización de HLTB fue cancelada.";
  if (error.code === "HLTB_DISABLED")
    return "Activa la integración para sincronizar duraciones.";
  if (error.code === "HLTB_OFFLINE")
    return "HowLongToBeat no responde. Inténtalo de nuevo más tarde.";
  if (error.code === "HLTB_API_ERROR" || error.code === "HLTB_INVALID_RESPONSE")
    return "La fuente de HowLongToBeat devolvió una respuesta no válida.";
  return "No se pudo sincronizar HowLongToBeat.";
}

function toAchievementSyncErrorMessage(error: unknown): string {
  if (!(error instanceof ProviderSettingsError))
    return "No se pudieron actualizar los trofeos de Steam.";
  if (error.code === "STEAM_OFFLINE")
    return "Steam no responde. Inténtalo de nuevo.";
  if (error.code === "STEAM_ACHIEVEMENT_SYNC_ALREADY_RUNNING")
    return "Ya hay una actualización de trofeos en curso.";
  if (error.code === "CREDENTIAL_ERROR")
    return "Credencial no disponible. Vuelve a introducir la API Key de Steam.";
  return "No se pudieron actualizar los trofeos de Steam.";
}

function toStorageErrorMessage(error: unknown): string {
  if (!(error instanceof ProviderSettingsError))
    return "No se pudo consultar el almacenamiento.";
  if (error.code === "STORAGE_MIGRATION_ALREADY_RUNNING")
    return "Ya hay una migración de almacenamiento en curso.";
  if (error.code === "STORAGE_MIGRATION_BUSY")
    return "Espera a que termine la sincronización actual antes de migrar.";
  if (error.code === "STORAGE_MIGRATION_SAME_MODE")
    return "La ubicación seleccionada ya está activa.";
  if (error.code === "STORAGE_MIGRATION_DATABASE_INVALID")
    return "La base de datos del destino no pudo validarse; el origen se conservó.";
  if (error.code === "STORAGE_MIGRATION_IO_ERROR")
    return "No se pudo escribir el destino. El origen sigue funcionando.";
  if (error.code === "STORAGE_MIGRATION_VALIDATION_ERROR")
    return "La copia no pasó la verificación. El origen sigue funcionando.";
  return "No se pudo completar la migración de almacenamiento.";
}
