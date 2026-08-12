import { useQuery } from "@tanstack/react-query";
import type { ResolvedGameCapabilities } from "../game-capabilities/game-capabilities-types";
import {
  graphicsProfileService,
  readDisplayCapabilities,
} from "./graphics-profile-service";
import {
  formatVram,
  hardwareCapabilitiesService,
  hardwareVendorLabel,
} from "./hardware-capabilities-service";
import type {
  HardwareCapabilities,
  RecommendedGraphicsProfile,
} from "./graphics-profile-types";

export function GraphicsProfilePanel({
  gameId,
  capabilities,
}: {
  gameId: string;
  capabilities: ResolvedGameCapabilities;
}) {
  const hardwareQuery = useQuery({
    queryKey: ["hardware-capabilities"],
    queryFn: () => hardwareCapabilitiesService.get(),
    staleTime: Infinity,
    refetchOnWindowFocus: false,
    retry: false,
  });
  const query = useQuery({
    queryKey: [
      "graphics-profile",
      gameId,
      capabilities.resolvedAt,
      hardwareQuery.data?.observedAt ?? 0,
    ],
    queryFn: async () =>
      graphicsProfileService.resolve(
        gameId,
        capabilities,
        await readDisplayCapabilities(gameId),
        hardwareQuery.data,
      ),
    enabled: hardwareQuery.isSuccess,
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
          Basado en tu hardware y la evidencia disponible
          <span aria-hidden="true">ⓘ</span>
        </span>
      </div>
      <div className="graphics-profile-panel">
        {query.isPending && (
          <p className="graphics-profile-status">Calculando recomendación…</p>
        )}
        {query.error && (
          <p className="graphics-profile-status">
            No hay suficiente información de hardware para recomendar un perfil.
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
  return (
    <div className="graphics-profile-content">
      <dl className="graphics-profile-list">
        <RecommendationItem
          label="Resolución objetivo"
          value={resolutionLabel(profile)}
          secondary={resolutionDescriptor(profile) ?? undefined}
          primary
        />
        <RecommendationItem
          label="Frecuencia"
          value={refreshRateLabel(profile)}
          primary
        />
        <RecommendationItem
          label="HDR"
          value={hdrLabel(profile.display.hdrMode)}
        />
        <RecommendationItem
          label="Upscaling"
          value={technologyLabel(
            profile.upscaling.technology,
            profile.upscaling.mode,
          )}
          secondary={upscalingModeLabel(profile.upscaling.mode) ?? undefined}
        />
        <RecommendationItem
          label="Frame Generation"
          value={frameGenerationLabel(profile)}
          secondary={frameGenerationStateLabel(profile.frameGeneration.mode)}
        />
      </dl>
      <div className="graphics-profile-meta-row">
        <p className="graphics-profile-display-note">
          Puedes ajustar la pantalla desde Configuración &gt; Pantalla
        </p>
        <div className="graphics-profile-meta-details">
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

function frameGenerationLabel(profile: RecommendedGraphicsProfile): string {
  const { frameGeneration } = profile;
  if (frameGeneration.technology) return frameGeneration.technology.label;
  if (frameGeneration.mode === "OFF") return "No disponible";
  if (frameGeneration.mode === "ALTERNATIVE_AVAILABLE") {
    return "Alternativa disponible";
  }
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
    : "Desconocido";
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
