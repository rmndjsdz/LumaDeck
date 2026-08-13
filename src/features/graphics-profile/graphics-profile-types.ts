import type { ResolvedGameCapabilities } from "../game-capabilities/game-capabilities-types";

export type HardwareVendor = "NVIDIA" | "AMD" | "INTEL" | "OTHER" | "UNKNOWN";

export type FeatureSupport = "SUPPORTED" | "UNSUPPORTED" | "UNKNOWN";

export type HardwareConfidence = "HIGH" | "MEDIUM" | "LOW";

export type HardwareFeatureSupport = {
  supportsDlss: FeatureSupport;
  supportsDlssFrameGeneration: FeatureSupport;
  supportsFsr: FeatureSupport;
  supportsFsrFrameGeneration: FeatureSupport;
  supportsXess: FeatureSupport;
  supportsXessFrameGeneration: FeatureSupport;
  preferredXess: FeatureSupport;
  supportsTsr: FeatureSupport;
  supportsNis: FeatureSupport;
};

export type GpuAdapter = {
  gpuId: string;
  vendor: HardwareVendor;
  vendorId: number | null;
  deviceId: number | null;
  model: string;
  dedicatedVramMb: number | null;
  architecture: string | null;
  driverVersion: string | null;
  luid: string | null;
  isSoftware: boolean;
  featureSupport: HardwareFeatureSupport;
  confidence: HardwareConfidence;
};

export type HardwareCapabilities = {
  gpuId: string | null;
  vendor: HardwareVendor;
  model: string | null;
  dedicatedVramMb: number | null;
  architecture: string | null;
  driverVersion: string | null;
  featureSupport: HardwareFeatureSupport;
  adapters: GpuAdapter[];
  preferredGamingGpu: GpuAdapter | null;
  confidence: HardwareConfidence;
  diagnostic: string | null;
  observedAt: number;
};

export type DisplayResolution = {
  width: number;
  height: number;
};

export type DisplayCapabilities = {
  displayId: string;
  currentResolution: DisplayResolution | null;
  supportedResolutions: DisplayResolution[];
  currentRefreshRate: number | null;
  supportedRefreshRates: number[];
  hdrSupported: boolean | null;
  hdrEnabled: boolean | null;
};

export type GraphicsProfileInput = {
  gameId: string;
  gameCapabilities: ResolvedGameCapabilities;
  hardware: HardwareCapabilities;
  display: DisplayCapabilities;
};

export type RecommendedGraphicsProfile = {
  gameId: string;
  source?: "NVIDIA_OPS" | "LUMADECK";
  sourceVersion?: string | null;
  popIndex?: number | null;
  belowMinSpec?: boolean;
  settings?: {
    canonicalKey: string;
    displayName: string;
    value: string;
    rawKey: string;
    rawValue: string;
  }[];
  display: {
    displayId: string;
    resolution: DisplayResolution | null;
    refreshRate: number | null;
    hdrMode:
      | "OFF"
      | "NATIVE"
      | "RTX_HDR_NATURAL"
      | "SYSTEM"
      | "AUTO"
      | "ALTERNATIVE_AVAILABLE"
      | "UNKNOWN";
  };
  upscaling: {
    mode: "RECOMMENDED" | "AUTO" | "NONE" | "UNKNOWN";
    modeLabel?: string | null;
    technology: {
      name: string;
      version: string | null;
      label: string;
    } | null;
  };
  frameGeneration: {
    mode: "NATIVE" | "OFF" | "ALTERNATIVE_AVAILABLE" | "UNKNOWN";
    modeLabel?: string | null;
    technology: {
      name: string;
      version: string | null;
      label: string;
    } | null;
  };
  losslessScaling: {
    recommendation: "NOT_RECOMMENDED" | "NOT_AVAILABLE" | "UNKNOWN";
  };
  confidence: "HIGH" | "MEDIUM" | "LOW";
  reasons: string[];
  warnings: string[];
  provenance: {
    resolution:
      | "PCGAMINGWIKI"
      | "NVIDIA_OPS"
      | "LOCAL_HARDWARE"
      | "LOCAL_DISPLAY"
      | "LUMADECK_RULE"
      | "UNKNOWN";
    refreshRate:
      | "PCGAMINGWIKI"
      | "NVIDIA_OPS"
      | "LOCAL_HARDWARE"
      | "LOCAL_DISPLAY"
      | "LUMADECK_RULE"
      | "UNKNOWN";
    hdr:
      | "PCGAMINGWIKI"
      | "NVIDIA_OPS"
      | "LOCAL_HARDWARE"
      | "LOCAL_DISPLAY"
      | "LUMADECK_RULE"
      | "UNKNOWN";
    upscaling:
      | "PCGAMINGWIKI"
      | "NVIDIA_OPS"
      | "LOCAL_HARDWARE"
      | "LOCAL_DISPLAY"
      | "LUMADECK_RULE"
      | "UNKNOWN";
    frameGeneration:
      | "PCGAMINGWIKI"
      | "NVIDIA_OPS"
      | "LOCAL_HARDWARE"
      | "LOCAL_DISPLAY"
      | "LUMADECK_RULE"
      | "UNKNOWN";
  };
};

export const unknownHardware: HardwareCapabilities = {
  gpuId: null,
  vendor: "UNKNOWN",
  model: null,
  dedicatedVramMb: null,
  architecture: null,
  driverVersion: null,
  featureSupport: {
    supportsDlss: "UNKNOWN",
    supportsDlssFrameGeneration: "UNKNOWN",
    supportsFsr: "UNKNOWN",
    supportsFsrFrameGeneration: "UNKNOWN",
    supportsXess: "UNKNOWN",
    supportsXessFrameGeneration: "UNKNOWN",
    preferredXess: "UNKNOWN",
    supportsTsr: "UNKNOWN",
    supportsNis: "UNKNOWN",
  },
  adapters: [],
  preferredGamingGpu: null,
  confidence: "LOW",
  diagnostic: "HARDWARE_NOT_LOADED",
  observedAt: 0,
};

export const unknownDisplay: DisplayCapabilities = {
  displayId: "unknown",
  currentResolution: null,
  supportedResolutions: [],
  currentRefreshRate: null,
  supportedRefreshRates: [],
  hdrSupported: null,
  hdrEnabled: null,
};
