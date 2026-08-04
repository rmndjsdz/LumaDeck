import { Focusable, type FocusableProps } from "../focus/Focusable";
import {
  availabilityMessage,
  type ActionAvailability,
  type FeedbackAvailability,
} from "./availability-types";

export interface AvailabilityFocusableProps extends Omit<
  FocusableProps,
  "disabled" | "onConfirm"
> {
  availability: ActionAvailability;
  onAvailable?: () => void;
  onAvailabilityFeedback?: (availability: FeedbackAvailability) => void;
}

export function AvailabilityFocusable({
  availability,
  onAvailable,
  onAvailabilityFeedback,
  ...props
}: AvailabilityFocusableProps) {
  const onConfirm = () => {
    if (availability === "available") {
      onAvailable?.();
      return;
    }
    if (availability === "coming-soon" || availability === "locked") {
      onAvailabilityFeedback?.(availability);
    }
  };

  return (
    <Focusable
      {...props}
      disabled={availability === "unavailable"}
      onConfirm={availability === "unavailable" ? undefined : onConfirm}
    />
  );
}

export interface AvailabilityFeedbackProps {
  availability: FeedbackAvailability;
  onDismiss: () => void;
}

export function AvailabilityFeedback({
  availability,
  onDismiss,
}: AvailabilityFeedbackProps) {
  const copy = availabilityMessage(availability);
  return (
    <div
      className="availability-feedback"
      role="status"
      aria-live="polite"
      data-availability-feedback={availability}
    >
      <span>
        <strong>{copy.title}</strong>
        <small>{copy.message}</small>
      </span>
      <button
        type="button"
        tabIndex={-1}
        onClick={onDismiss}
        aria-label="Cerrar mensaje"
      >
        ×
      </button>
    </div>
  );
}
