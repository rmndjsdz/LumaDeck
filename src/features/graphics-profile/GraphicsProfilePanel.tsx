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
      className="graphics-profile-panel"
      aria-labelledby="graphics-profile-heading"
    >
      <p className="eyebrow">Recomendado para este equipo</p>
      <h3 id="graphics-profile-heading">Perfil gráfico sugerido</h3>
      {query.isPending && (
        <p className="graphics-profile-status">Calculando recomendación…</p>
      )}
      {query.error && (
        <p className="graphics-profile-status">
          No hay suficiente información de hardware para recomendar un perfil.
        </p>
      )}
      {query.data && <RecommendationContent profile={query.data} />}
      {hardwareQuery.data && <HardwareSummary hardware={hardwareQuery.data} />}
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
}: {
  profile: RecommendedGraphicsProfile;
}) {
  return (
    <>
      <dl className="graphics-profile-list">
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
        />
        <RecommendationItem
          label="Frame Generation"
          value={frameGenerationLabel(profile)}
        />
        <RecommendationItem label="Pantalla" value={displayLabel(profile)} />
      </dl>
      {profile.warnings.length > 0 && (
        <ul className="graphics-profile-warnings">
          {profile.warnings.map((warning) => (
            <li key={warning}>{warning}</li>
          ))}
        </ul>
      )}
      <details className="graphics-profile-reasons">
        <summary>¿Por qué? · Confianza {profile.confidence}</summary>
        <ul>
          {profile.reasons.map((reason) => (
            <li key={reason}>{reason}</li>
          ))}
        </ul>
      </details>
    </>
  );
}

function RecommendationItem({
  label,
  value,
}: {
  label: string;
  value: string;
}) {
  return (
    <div>
      <dt>{label}</dt>
      <dd>{value}</dd>
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

function displayLabel(profile: RecommendedGraphicsProfile): string {
  const resolution = profile.display.resolution;
  const size = resolution
    ? `${resolution.width} × ${resolution.height}`
    : "Auto";
  const refresh = profile.display.refreshRate
    ? ` @ ${profile.display.refreshRate} Hz`
    : "";
  return `${size}${refresh}`;
}
