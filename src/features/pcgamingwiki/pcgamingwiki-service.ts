import { invoke } from "@tauri-apps/api/core";
import type {
  PcgamingwikiCapabilitiesResponse,
  PcgamingwikiIdentity,
  PcgamingwikiRequestOptions,
} from "./pcgamingwiki-types";

export const pcgamingwikiService = {
  getCapabilities(
    gameId: string,
    identity: PcgamingwikiIdentity,
    options: PcgamingwikiRequestOptions = {},
  ): Promise<PcgamingwikiCapabilitiesResponse> {
    if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) {
      return Promise.resolve({
        status: "IDENTITY_UNAVAILABLE",
        gameRef: null,
        capabilities: null,
        source: "PCGAMINGWIKI",
        providerVersion: 1,
        stale: false,
        conflict: null,
        error: "PCGW_DESKTOP_RUNTIME_UNAVAILABLE",
      });
    }
    return invoke<PcgamingwikiCapabilitiesResponse>(
      "get_pcgamingwiki_capabilities",
      {
        gameId,
        steamAppId: identity.steamAppId ?? null,
        gogProductId: identity.gogProductId ?? null,
        forceRefresh: options.forceRefresh ?? false,
        crossCheckIdentities: options.crossCheckIdentities ?? false,
      },
    );
  },
};

export function pcgamingwikiStatusMessage(
  status: PcgamingwikiCapabilitiesResponse["status"],
): string {
  switch (status) {
    case "NOT_FOUND":
      return "PCGamingWiki no tiene una página para este identificador.";
    case "PCGW_FORBIDDEN":
      return "PCGamingWiki rechazó la consulta; se usará la caché si existe.";
    case "IDENTITY_UNAVAILABLE":
      return "Este juego no tiene un Steam App ID o GOG Product ID utilizable.";
    case "RATE_LIMITED":
      return "PCGamingWiki limitó temporalmente las consultas.";
    case "TIMEOUT":
      return "PCGamingWiki tardó demasiado en responder.";
    case "NETWORK_ERROR":
      return "PCGamingWiki no está disponible; se usará la caché si existe.";
    case "INVALID_REDIRECT":
      return "PCGamingWiki devolvió un destino no válido.";
    case "PARSE_FAILURE":
      return "La página de PCGamingWiki no pudo interpretarse.";
    case "TEMPORARY_FAILURE":
      return "PCGamingWiki devolvió un error temporal.";
    case "RESOLVED":
      return "Datos técnicos de PCGamingWiki actualizados.";
  }
}
