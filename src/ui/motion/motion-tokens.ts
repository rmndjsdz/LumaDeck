export const motionTokens = {
  duration: {
    instant: 88,
    focusFast: 132,
    standard: 184,
    panel: 212,
    viewEnter: 248,
    viewExit: 168,
    backgroundCrossfade: 304,
  },
  easing: {
    standard: "cubic-bezier(0.2, 0.8, 0.2, 1)",
    enter: "cubic-bezier(0.16, 1, 0.3, 1)",
    exit: "cubic-bezier(0.4, 0, 1, 1)",
    focus: "cubic-bezier(0.18, 0.9, 0.25, 1)",
  },
} as const;

export type MotionTokenName = keyof typeof motionTokens.duration;
