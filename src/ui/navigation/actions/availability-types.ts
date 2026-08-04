export type ActionAvailability =
  "available" | "coming-soon" | "locked" | "unavailable";

export type FeedbackAvailability = Exclude<
  ActionAvailability,
  "available" | "unavailable"
>;

export function availabilityMessage(availability: FeedbackAvailability): {
  title: string;
  message: string;
} {
  if (availability === "locked") {
    return {
      title: "Bloqueado",
      message: "Esta opción estará disponible cuando se cumpla su condición.",
    };
  }
  return {
    title: "Próximamente",
    message: "Esta función llegará en una próxima versión.",
  };
}
