# Product slice

The first product slice keeps the navigation engine as the only owner of
focus and input. The shell is persistent while Home, Library and Details swap
their page content inside the same `product-shell` scope.

## Data

`src/features/catalog/mock-catalog.ts` creates exactly 200 deterministic local
games. `useGames` exposes that catalog through TanStack Query with an infinite
stale time; there is no network, Steam integration or launcher process yet.
The product store only keeps the active view, selected game and the focus/view
needed to return from Details.

## Library rendering

Library uses a deterministic title/status/sort pipeline and a 60-card window.
Cards keep their logical index in the navigation registry. When a move targets
an unmounted index, the engine asks Library to shift the window, then focuses
the requested card after React commits. The CSS grid remains five columns so
the visual and logical grids cannot diverge at responsive breakpoints.

The library scroll container is registered with `ScrollRestoration`, so
switching views remembers and restores its exact `scrollTop`/`scrollLeft`.

## Backgrounds and input

`BackgroundManager` preloads the next local SVG before committing it and
crossfades the current and incoming layers. A failed preload leaves the current
background untouched. `AutoCursor` hides the cursor after keyboard/gamepad
input and reveals it on pointer movement.

Development builds expose navigation and performance overlays. The gamepad
adapter reports connection, direction and pressed-button diagnostics only when
those values change; it does not publish analog state every frame.
