export function formatHltbDuration(minutes: number | null | undefined): string {
  if (minutes === null || minutes === undefined || minutes <= 0)
    return "Sin datos";
  const hours = Math.max(1, Math.round(minutes / 60));
  return `${hours} h`;
}
