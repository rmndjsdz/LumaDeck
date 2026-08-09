import { invoke } from "@tauri-apps/api/core";
import type {
  GameCapabilityIdentity,
  GameCapabilityKind,
  GameCapabilityOverrideState,
  ResolvedGameCapabilities,
} from "./game-capabilities-types";

function isDesktopRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function emptyCapabilities(gameId: string): ResolvedGameCapabilities {
  const empty = (kind: GameCapabilityKind) => ({
    kind,
    value: "UNKNOWN" as const,
    confidence: "LOW" as const,
    source: "NONE" as const,
    technologies: [],
    alternativeAvailable: "UNKNOWN" as const,
    sourceNote: null,
    evidence: null,
    otherEvidence: [],
    resolvedAt: Date.now(),
    stale: false,
    hasConflict: false,
  });
  return {
    gameId,
    nativeHdr: empty("NATIVE_HDR"),
    highFidelityUpscaling: empty("HIGH_FIDELITY_UPSCALING"),
    frameGeneration: empty("FRAME_GENERATION"),
    resolvedAt: Date.now(),
    providerStatus: "IDENTITY_UNAVAILABLE",
    providerError: "GAME_CAPABILITIES_DESKTOP_RUNTIME_UNAVAILABLE",
  };
}

function identityPayload(identity: GameCapabilityIdentity) {
  return {
    steamAppId: identity.steamAppId ?? null,
    gogProductId: identity.gogProductId ?? null,
  };
}

export const gameCapabilitiesService = {
  get(
    gameId: string,
    identity: GameCapabilityIdentity,
  ): Promise<ResolvedGameCapabilities> {
    if (!isDesktopRuntime()) return Promise.resolve(emptyCapabilities(gameId));
    return invoke<ResolvedGameCapabilities>("get_game_capabilities", {
      gameId,
      ...identityPayload(identity),
    });
  },

  refresh(
    gameId: string,
    identity: GameCapabilityIdentity,
  ): Promise<ResolvedGameCapabilities> {
    if (!isDesktopRuntime()) return Promise.resolve(emptyCapabilities(gameId));
    return invoke<ResolvedGameCapabilities>("refresh_game_capabilities", {
      gameId,
      ...identityPayload(identity),
    });
  },

  setOverride(
    gameId: string,
    capability: GameCapabilityKind,
    overrideState: Exclude<GameCapabilityOverrideState, "NO_OVERRIDE">,
  ): Promise<ResolvedGameCapabilities> {
    if (!isDesktopRuntime()) return Promise.resolve(emptyCapabilities(gameId));
    return invoke<ResolvedGameCapabilities>("set_game_capability_override", {
      gameId,
      capability,
      overrideState,
    });
  },

  clearOverride(
    gameId: string,
    capability: GameCapabilityKind,
  ): Promise<ResolvedGameCapabilities> {
    if (!isDesktopRuntime()) return Promise.resolve(emptyCapabilities(gameId));
    return invoke<ResolvedGameCapabilities>("clear_game_capability_override", {
      gameId,
      capability,
    });
  },
};

export function capabilityValueLabel(
  value: ResolvedGameCapabilities["nativeHdr"]["value"],
): string {
  if (value === "YES") return "Sí";
  if (value === "NO") return "No";
  return "Desconocido";
}

export function capabilitySourceLabel(
  source: ResolvedGameCapabilities["nativeHdr"]["source"],
): string {
  if (source === "PCGAMINGWIKI") return "PCGamingWiki";
  if (source === "USER_OVERRIDE") return "Usuario";
  return "Sin evidencia";
}
