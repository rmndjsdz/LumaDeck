export const PCGAMINGWIKI_PROVIDER_VERSION = 1 as const;

export type PcgamingwikiResolutionStatus =
  | "RESOLVED"
  | "NOT_FOUND"
  | "PCGW_FORBIDDEN"
  | "IDENTITY_UNAVAILABLE"
  | "RATE_LIMITED"
  | "NETWORK_ERROR"
  | "TIMEOUT"
  | "TEMPORARY_FAILURE"
  | "INVALID_REDIRECT"
  | "PARSE_FAILURE";

export type PcgamingwikiResolvedVia =
  "MEDIAWIKI_STEAM_ID" | "MEDIAWIKI_GOG_ID" | "STEAM_APP_ID" | "GOG_PRODUCT_ID";
export type PcgamingwikiNormalizedValue = "YES" | "NO" | "UNKNOWN";
export type PcgamingwikiConfidence = "HIGH" | "MEDIUM" | "LOW";
export type PcgamingwikiCapability =
  "NATIVE_HDR" | "HIGH_FIDELITY_UPSCALING" | "FRAME_GENERATION";

export type PcgamingwikiGameRef = {
  pageTitle: string;
  pageId: string | null;
  canonicalUrl: string;
  steamAppId: number | null;
  gogProductId: string | null;
  resolvedVia: PcgamingwikiResolvedVia;
  resolvedAt: string;
  redirectChain: string[];
};

export type PcgamingwikiCapabilityEvidence = {
  capability: PcgamingwikiCapability;
  normalizedValue: PcgamingwikiNormalizedValue;
  sourceValue: string | null;
  alternativeAvailable: PcgamingwikiNormalizedValue;
  sourceNote: string | null;
  technologies: string[];
  source: "PCGAMINGWIKI" | string;
  sourcePage: string;
  sourceField: string;
  confidence: PcgamingwikiConfidence;
  observedAt: string;
  providerVersion: number;
  stale: boolean;
};

export type PcgamingwikiCapabilities = {
  nativeHdr: PcgamingwikiCapabilityEvidence;
  highFidelityUpscaling: PcgamingwikiCapabilityEvidence;
  frameGeneration: PcgamingwikiCapabilityEvidence;
};

export type PcgamingwikiIdentityConflict = {
  steam: PcgamingwikiGameRef;
  gog: PcgamingwikiGameRef;
  code: "PCGW_IDENTITY_CONFLICT" | string;
};

export type PcgamingwikiCapabilitiesResponse = {
  status: PcgamingwikiResolutionStatus;
  gameRef: PcgamingwikiGameRef | null;
  capabilities: PcgamingwikiCapabilities | null;
  source: "PCGAMINGWIKI" | string;
  providerVersion: number;
  stale: boolean;
  conflict: PcgamingwikiIdentityConflict | null;
  error: string | null;
};

export type PcgamingwikiIdentity = {
  steamAppId?: number | null;
  gogProductId?: string | null;
};

export type PcgamingwikiRequestOptions = {
  forceRefresh?: boolean;
  crossCheckIdentities?: boolean;
};
