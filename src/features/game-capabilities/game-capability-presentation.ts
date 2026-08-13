import type { ResolvedCapability } from "./game-capabilities-types";

export function capabilityStateLabel(capability: ResolvedCapability): string {
  if (capability.value === "YES") return "Compatible";
  if (capability.value === "NO") return "No compatible";
  return "Desconocido";
}

export function capabilityStateClass(capability: ResolvedCapability): string {
  return `is-${capability.value.toLowerCase()}`;
}
