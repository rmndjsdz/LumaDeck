# Graphics Profile Resolver V1

Graphics Profile Resolver V1 is a pure, recommendation-only layer. It consumes
the resolved `GameCapabilities`, normalized `HardwareCapabilities`, and the
selected display's `DisplayCapabilities`. It returns a deterministic
`RecommendedGraphicsProfile`; it does not call PCGamingWiki, probe hardware,
persist the result, launch a game, or apply a display/driver/game setting.

## Inputs and output

Hardware is explicit and tri-state (`true`, `false`, or unknown) for DLSS,
XeSS, FSR, TSR, NIS, and their frame-generation variants. GPU vendor is a
closed enum with an `UNKNOWN` value. Display input includes the selected
`displayId`, current mode, supported modes, and HDR support/state.

The output preserves the current resolution and refresh rate when the source
is the LumaDeck fallback, and records per-field provenance. It contains HDR,
upscaling, frame-generation, and Lossless Scaling recommendations, an overall
confidence, auditable reason strings, and warnings. Technology labels retain
versions such as `DLSS 4`, `FSR 4`, and `XeSS 2`.

## Rules

- Unsupported display HDR always yields `OFF`; a game reporting native HDR does
  not override the display.
- Native HDR `YES` plus display HDR support yields `NATIVE`.
- Native HDR `NO` yields `OFF`; if evidence says an alternative/workaround is
  available, the reason and warning preserve that fact. V1 does not execute it.
- Unknown HDR evidence or display state remains `UNKNOWN`.
- Native HDR capability and HDR recommendation are separate: a compatible local
  NVIDIA/display combination may recommend RTX HDR while the game capability
  remains `NATIVE_HDR=NO`.
- Upscaling selection follows the conservative matrix NVIDIA (DLSS, XeSS, FSR,
  TSR, NIS), AMD (FSR, XeSS, TSR), and Intel (XeSS, FSR, TSR). A technology is
  selected only when both the game lists it and the normalized hardware matrix
  explicitly confirms support.
- Frame generation requires an explicit compatible hardware feature. A native
  game `NO` is not converted into a Lossless Scaling action, even when a
  workaround note exists.
- `USER_OVERRIDE` is consumed through the already-resolved capability and is
  visible in the reason trace; this resolver does not access the override table.

## Current hardware limitation

The repository has display discovery but no reliable normalized GPU capability
source. The Details UI therefore passes an `UNKNOWN` hardware context until a
trusted detector is added. The resolver and unit fixtures already cover NVIDIA,
AMD, Intel, and unknown hardware without hardcoded model-name checks.

Future V2 may add RTX HDR, Auto HDR, engine/workaround execution, performance
telemetry, target FPS, VRR, and game configuration adapters. Those are outside
V1.
