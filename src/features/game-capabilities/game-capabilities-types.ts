export type GameCapabilityKind =
  | "NATIVE_HDR"
  | "HIGH_FIDELITY_UPSCALING"
  | "FRAME_GENERATION"
  | "FOUR_K"
  | "SIXTY_FPS"
  | "HIGH_REFRESH_120_FPS";

export type GameCapabilityValue = "YES" | "NO" | "UNKNOWN";

export type GameCapabilityConfidence =
  "HIGH" | "MEDIUM" | "LOW" | "USER_DEFINED";

export type GameCapabilitySource = "PCGAMINGWIKI" | "USER_OVERRIDE" | "NONE";

export type GameCapabilityOverrideState =
  "NO_OVERRIDE" | "FORCE_YES" | "FORCE_NO" | "FORCE_UNKNOWN";

export type GameCapabilityEvidence = {
  gameId: string;
  capability: GameCapabilityKind;
  value: GameCapabilityValue;
  source: GameCapabilitySource;
  sourceValue: string | null;
  alternativeAvailable: GameCapabilityValue;
  sourceNote: string | null;
  confidence: GameCapabilityConfidence;
  technologies: string[];
  observedAt: string;
  sourceReference: string | null;
  providerVersion: number | null;
  stale: boolean;
};

export type ResolvedCapability = {
  kind: GameCapabilityKind;
  value: GameCapabilityValue;
  confidence: GameCapabilityConfidence;
  source: GameCapabilitySource;
  technologies: string[];
  alternativeAvailable: GameCapabilityValue;
  sourceNote: string | null;
  evidence: GameCapabilityEvidence | null;
  otherEvidence: GameCapabilityEvidence[];
  resolvedAt: number;
  stale: boolean;
  hasConflict: boolean;
};

export type ResolvedGameCapabilities = {
  gameId: string;
  nativeHdr: ResolvedCapability;
  highFidelityUpscaling: ResolvedCapability;
  frameGeneration: ResolvedCapability;
  fourK: ResolvedCapability;
  sixtyFps: ResolvedCapability;
  highRefresh120Fps: ResolvedCapability;
  resolvedAt: number;
  providerStatus: string | null;
  providerError: string | null;
};

export type GameCapabilityIdentity = {
  steamAppId?: number | null;
  gogProductId?: string | null;
};
