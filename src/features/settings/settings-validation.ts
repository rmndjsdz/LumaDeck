export interface ValidationResult {
  value?: string;
  error?: string;
}

export function validateSteamId64(value: string): ValidationResult {
  const normalized = value.trim();
  if (!normalized) return { error: "Ingresa tu SteamID64." };
  if (!/^\d+$/.test(normalized)) {
    return { error: "El SteamID64 solo puede contener dígitos." };
  }
  if (normalized.length !== 17) {
    return { error: "El SteamID64 debe tener 17 dígitos." };
  }
  return { value: normalized };
}

export function validateSteamApiKey(value: string): ValidationResult {
  const normalized = value.trim();
  if (!normalized) return { error: "Ingresa tu Steam Web API Key." };
  if (!/^[A-Za-z0-9_-]+$/.test(normalized)) {
    return { error: "La API Key contiene caracteres no válidos." };
  }
  if (normalized.length < 16 || normalized.length > 64) {
    return { error: "La API Key debe tener entre 16 y 64 caracteres." };
  }
  return { value: normalized };
}
