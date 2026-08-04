export const motionTokens = {
  duration: {
    instant: 88,
    focusFast: 160,
    standard: 192,
    panel: 192,
    keyboardEnter: 176,
    viewEnter: 275,
    viewExit: 176,
    backgroundCrossfade: 280,
    homeStagger: 14,
    homeScene: 400,
  },
  easing: {
    standard: "cubic-bezier(0.2, 0.8, 0.2, 1)",
    enter: "cubic-bezier(0.16, 1, 0.3, 1)",
    exit: "cubic-bezier(0.4, 0, 1, 1)",
    focus: "cubic-bezier(0.22, 0.9, 0.3, 1.01)",
  },
} as const;

export type MotionTokenName = keyof typeof motionTokens.duration;
