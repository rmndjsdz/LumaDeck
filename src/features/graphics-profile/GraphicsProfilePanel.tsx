import { useQuery } from "@tanstack/react-query";
import type { ResolvedGameCapabilities } from "../game-capabilities/game-capabilities-types";
import {
  graphicsProfileService,
  readDisplayCapabilities,
} from "./graphics-profile-service";
import {
  hasUsableNvidiaOpsProfile,
  nvidiaOpsService,
} from "./nvidia-ops-service";
import type { NvidiaOpsProfile } from "./nvidia-ops-types";
import {
  formatVram,
  hardwareCapabilitiesService,
  hardwareVendorLabel,
} from "./hardware-capabilities-service";
import type {
  DisplayCapabilities,
  HardwareCapabilities,
  RecommendedGraphicsProfile,
} from "./graphics-profile-types";
import { unknownDisplay } from "./graphics-profile-types";

type ProfileSource = "NVIDIA_OPS" | "LUMADECK";

export function GraphicsProfilePanel({
  gameId,
  steamAppId,
  title,
  executablePath,
  capabilities,
}: {
  gameId: string;
  steamAppId: number | null;
  title: string | null;
  executablePath: string | null;
  capabilities: ResolvedGameCapabilities;
}) {
  const hardwareQuery = useQuery({
    queryKey: ["hardware-capabilities"],
    queryFn: () => hardwareCapabilitiesService.get(),
    staleTime: Infinity,
    refetchOnWindowFocus: false,
    retry: false,
  });
  const displayQuery = useQuery({
    queryKey: ["graphics-profile-display", gameId],
    queryFn: () => readDisplayCapabilities(gameId),
    staleTime: Infinity,
    refetchOnWindowFocus: false,
    retry: false,
  });
  const query = useQuery({
    queryKey: [
      "graphics-profile",
      gameId,
      steamAppId,
      capabilities.resolvedAt,
      hardwareQuery.data?.observedAt ?? 0,
      displayQuery.data?.currentResolution?.width ?? 0,
      displayQuery.data?.currentResolution?.height ?? 0,
    ],
    queryFn: async () => {
      const display = displayQuery.data ?? unknownDisplay;
      try {
        const ops = await nvidiaOpsService.get(
          gameId,
          steamAppId,
          title,
          executablePath,
          display.currentResolution,
        );
        if (hasUsableNvidiaOpsProfile(ops)) {
          return profileFromNvidiaOps(gameId, ops.profile, display);
        }
      } catch {
        // OPS is optional. The existing resolver remains the product fallback.
      }
      const fallback = await graphicsProfileService.resolve(
        gameId,
        capabilities,
        display,
        hardwareQuery.data,
      );
      return { ...fallback, source: "LUMADECK" as const };
    },
    enabled: displayQuery.isSuccess || displayQuery.isError,
    staleTime: 7 * 24 * 60 * 60 * 1000,
    refetchOnWindowFocus: false,
    retry: false,
  });

  return (
    <section
      className="graphics-profile-section"
      aria-labelledby="graphics-profile-heading"
    >
      <div className="graphics-profile-section-heading">
        <div>
          <p className="eyebrow">Perfil gráfico sugerido</p>
          <h3 id="graphics-profile-heading" className="visually-hidden">
            Perfil gráfico sugerido
          </h3>
        </div>
        <span className="graphics-profile-context">
          {query.data?.source === "NVIDIA_OPS"
            ? "Recomendado por NVIDIA"
            : "Basado en tu hardware y la evidencia disponible"}
          <span aria-hidden="true">ⓘ</span>
        </span>
      </div>
      <div className="graphics-profile-panel">
        {query.isPending && (
          <p className="graphics-profile-status">Calculando recomendación…</p>
        )}
        {query.error && (
          <p className="graphics-profile-status">
            No hay suficiente información para recomendar un perfil.
          </p>
        )}
        {query.data && (
          <RecommendationContent
            profile={query.data}
            hardware={hardwareQuery.data}
          />
        )}
        {!query.data && hardwareQuery.data && (
          <HardwareSummary hardware={hardwareQuery.data} />
        )}
      </div>
    </section>
  );
}

function profileFromNvidiaOps(
  gameId: string,
  profile: NvidiaOpsProfile,
  display: DisplayCapabilities,
): RecommendedGraphicsProfile & { source: ProfileSource } {
  const upscalingMode =
    settingValue(profile, "dlssSuperResolution") ??
    settingValue(profile, "fsr1") ??
    settingValue(profile, "fsr3");
  const upscalingTechnology = settingValue(profile, "upscalingTechnology");
  const frameGeneration =
    settingValue(profile, "dlssFrameGeneration") ??
    settingValue(profile, "fsrFrameGeneration");
  const frameGenerationTechnology = frameGeneration
    ? profile.settings.find(
        (setting) =>
          setting.canonicalKey === "dlssFrameGeneration" ||
          setting.canonicalKey === "fsrFrameGeneration",
      )
    : undefined;

  return {
    gameId,
    source: "NVIDIA_OPS",
    sourceVersion: profile.sourceVersion,
    popIndex: profile.popIndex,
    belowMinSpec: profile.belowMinSpec,
    settings: profile.settings,
    display: {
      displayId: display.displayId,
      resolution: profile.resolution,
      refreshRate: null,
      hdrMode: "UNKNOWN",
    },
    upscaling: {
      mode: upscalingMode ? "RECOMMENDED" : "UNKNOWN",
      modeLabel: upscalingMode,
      technology: upscalingTechnology
        ? technologyFromValue(upscalingTechnology)
        : null,
    },
    frameGeneration: {
      mode: frameGeneration
        ? isDisabledValue(frameGeneration)
          ? "OFF"
          : "NATIVE"
        : "UNKNOWN",
      modeLabel: frameGeneration,
      technology: frameGenerationTechnology
        ? technologyFromKey(frameGenerationTechnology.canonicalKey)
        : null,
    },
    losslessScaling: { recommendation: "NOT_AVAILABLE" },
    confidence: profile.confidence,
    provenance: {
      resolution: "NVIDIA_OPS",
      refreshRate: "UNKNOWN",
      hdr: "UNKNOWN",
      upscaling: "NVIDIA_OPS",
      frameGeneration: "NVIDIA_OPS",
    },
    reasons: [
      `NVIDIA OPS seleccionó el POP ${profile.popIndex} desde metadata local.`,
      profile.sourceVersion
        ? `Versión local de NVIDIA: ${profile.sourceVersion}.`
        : "El paquete local de NVIDIA no expone versión.",
    ],
    warnings: profile.belowMinSpec
      ? [
          "NVIDIA marca este equipo por debajo del objetivo recomendado para este juego.",
        ]
      : [],
  };
}

function settingValue(profile: NvidiaOpsProfile, key: string): string | null {
  return (
    profile.settings.find((setting) => setting.canonicalKey === key)?.value ??
    null
  );
}

function technologyFromValue(value: string) {
  const normalized = value.toLowerCase();
  if (normalized.includes("dlss")) {
    return { name: "DLSS", version: null, label: value };
  }
  if (normalized.includes("fsr")) {
    return { name: "FSR", version: null, label: value };
  }
  if (normalized.includes("xess")) {
    return { name: "XeSS", version: null, label: value };
  }
  return { name: "OPS_UPSCALING", version: null, label: value };
}

function technologyFromKey(key: string) {
  if (key === "fsrFrameGeneration") {
    return {
      name: "FSR_FRAME_GENERATION",
      version: null,
      label: "FSR Frame Generation",
    };
  }
  return {
    name: "DLSS_FRAME_GENERATION",
    version: null,
    label: "NVIDIA RTX Frame Generation",
  };
}

function isDisabledValue(value: string): boolean {
  return ["off", "disabled", "no"].includes(value.trim().toLowerCase());
}

function HardwareSummary({ hardware }: { hardware: HardwareCapabilities }) {
  const preferred = hardware.preferredGamingGpu;
  return (
    <p className="graphics-profile-hardware">
      GPU: {preferred?.model ?? hardware.model ?? "Desconocida"} ·{" "}
      {hardwareVendorLabel(hardware.vendor)} ·{" "}
      {formatVram(hardware.dedicatedVramMb)}
      {hardware.adapters.length > 1
        ? ` · ${hardware.adapters.length} adaptadores detectados`
        : ""}
    </p>
  );
}

function RecommendationContent({
  profile,
  hardware,
}: {
  profile: RecommendedGraphicsProfile;
  hardware?: HardwareCapabilities;
}) {
  const sourceLabel = profile.source === "NVIDIA_OPS" ? "NVIDIA OPS" : null;
  return (
    <div className="graphics-profile-content">
      {sourceLabel && (
        <p className="graphics-profile-source" aria-label="Fuente del perfil">
          Fuente: {sourceLabel}
        </p>
      )}
      <dl className="graphics-profile-list">
        <RecommendationItem
          label={
            profile.provenance.resolution === "LOCAL_DISPLAY"
              ? "Resolución de pantalla"
              : "Resolución objetivo"
          }
          value={resolutionLabel(profile)}
          secondary={resolutionDescriptor(profile) ?? undefined}
          primary
        />
        <RecommendationItem
          label={
            profile.provenance.refreshRate === "LOCAL_DISPLAY"
              ? "Frecuencia de pantalla"
              : "Frecuencia"
          }
          value={refreshRateLabel(profile)}
          primary
        />
        <RecommendationItem
          label="HDR"
          value={hdrLabel(profile.display.hdrMode)}
          secondary={hdrRecommendationSecondary(profile)}
        />
        <RecommendationItem
          label="Upscaling"
          value={technologyLabel(
            profile.upscaling.technology,
            profile.upscaling.mode,
          )}
          secondary={
            profile.upscaling.modeLabel ??
            upscalingModeLabel(profile.upscaling.mode) ??
            undefined
          }
        />
        <RecommendationItem
          label="Frame Generation"
          value={frameGenerationLabel(profile)}
          secondary={
            profile.frameGeneration.modeLabel ??
            frameGenerationStateLabel(profile.frameGeneration.mode)
          }
        />
      </dl>
      <div className="graphics-profile-meta-row">
        <p className="graphics-profile-display-note">
          Puedes ajustar la pantalla desde Configuración &gt; Pantalla
        </p>
        <div className="graphics-profile-meta-details">
          {profile.belowMinSpec && (
            <p className="graphics-profile-below-min-spec">
              NVIDIA marca este equipo por debajo del objetivo recomendado para
              este juego.
            </p>
          )}
          {profile.warnings.length > 0 && (
            <details className="graphics-profile-warnings">
              <summary>Advertencias ({profile.warnings.length})</summary>
              <ul>
                {profile.warnings.map((warning) => (
                  <li key={warning}>{warning}</li>
                ))}
              </ul>
            </details>
          )}
          <details className="graphics-profile-reasons">
            <summary>
              ¿Por qué? · Confianza {confidenceLabel(profile.confidence)}
            </summary>
            <ul>
              {profile.reasons.map((reason) => (
                <li key={reason}>{reason}</li>
              ))}
            </ul>
          </details>
          {hardware && <HardwareSummary hardware={hardware} />}
        </div>
      </div>
    </div>
  );
}

function RecommendationItem({
  label,
  value,
  secondary,
  primary = false,
}: {
  label: string;
  value: string;
  secondary?: string;
  primary?: boolean;
}) {
  return (
    <div className={primary ? "is-primary" : undefined}>
      <dt>{label}</dt>
      <dd>{value}</dd>
      {secondary && (
        <span className="graphics-profile-secondary">{secondary}</span>
      )}
    </div>
  );
}

function hdrLabel(
  mode: RecommendedGraphicsProfile["display"]["hdrMode"],
): string {
  if (mode === "NATIVE") return "HDR nativo";
  if (mode === "RTX_HDR_NATURAL") return "RTX HDR Natural";
  if (mode === "SYSTEM") return "System";
  if (mode === "ALTERNATIVE_AVAILABLE") return "Alternativa disponible";
  if (mode === "OFF") return "No disponible (SDR)";
  if (mode === "AUTO") return "Automático";
  return "Desconocido";
}

function technologyLabel(
  technology: RecommendedGraphicsProfile["upscaling"]["technology"],
  mode: RecommendedGraphicsProfile["upscaling"]["mode"],
): string {
  if (technology) return technology.label;
  if (mode === "NONE") return "No disponible";
  if (mode === "AUTO") return "Automático (GPU desconocida)";
  return "Desconocido";
}

function hdrRecommendationSecondary(
  profile: RecommendedGraphicsProfile,
): string | undefined {
  if (profile.display.hdrMode === "RTX_HDR_NATURAL") {
    return "Alternativa al HDR nativo";
  }
  if (profile.display.hdrMode === "NATIVE") {
    return "Soportado por el juego";
  }
  return undefined;
}

function frameGenerationLabel(profile: RecommendedGraphicsProfile): string {
  const { frameGeneration } = profile;
  if (frameGeneration.technology) return frameGeneration.technology.label;
  if (frameGeneration.mode === "OFF") return "No disponible";
  if (frameGeneration.mode === "ALTERNATIVE_AVAILABLE")
    return "Alternativa disponible";
  return "Desconocido";
}

function resolutionLabel(profile: RecommendedGraphicsProfile): string {
  const resolution = profile.display.resolution;
  return resolution
    ? `${resolution.width} × ${resolution.height}`
    : "Desconocido";
}

function refreshRateLabel(profile: RecommendedGraphicsProfile): string {
  return profile.display.refreshRate
    ? `${profile.display.refreshRate} Hz`
    : "No especificada";
}

function resolutionDescriptor(
  profile: RecommendedGraphicsProfile,
): string | null {
  const resolution = profile.display.resolution;
  if (!resolution) return null;
  if (resolution.width === 3840 && resolution.height === 2160) return "4K UHD";
  if (resolution.width === 2560 && resolution.height === 1440) return "1440p";
  if (resolution.width === 1920 && resolution.height === 1080) return "1080p";
  return `${resolution.width} × ${resolution.height}`;
}

function upscalingModeLabel(
  mode: RecommendedGraphicsProfile["upscaling"]["mode"],
): string | null {
  if (mode === "RECOMMENDED") return "Equilibrado";
  if (mode === "AUTO") return "Automático";
  if (mode === "NONE") return "No disponible";
  return null;
}

function frameGenerationStateLabel(
  mode: RecommendedGraphicsProfile["frameGeneration"]["mode"],
): string {
  if (mode === "NATIVE") return "Activado";
  if (mode === "OFF") return "No recomendado";
  if (mode === "ALTERNATIVE_AVAILABLE") return "Alternativa disponible";
  return "Desconocido";
}

function confidenceLabel(
  confidence: RecommendedGraphicsProfile["confidence"],
): string {
  if (confidence === "HIGH") return "Alta";
  if (confidence === "MEDIUM") return "Media";
  return "Baja";
}
