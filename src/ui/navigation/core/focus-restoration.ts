import type { FocusRegistry } from "./focus-registry";

export function canRestoreFocus(
  registry: FocusRegistry,
  focusId: string | undefined,
): focusId is string {
  if (!focusId) return false;
  const entry = registry.get(focusId);
  return Boolean(
    entry && !entry.disabled && !entry.hidden && entry.element.isConnected,
  );
}
