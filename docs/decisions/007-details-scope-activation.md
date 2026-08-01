# Details scope activation

## Root cause

Library and Details share one navigation engine. A virtual grid request could
remain pending after Library was replaced by Details. Because Details buttons
were registered in `product-shell`, a later directional action could still be
resolved using Library's pending grid. Independently, a scope with no
registered focusables was activated immediately and never retried its initial
focus.

## Invariants

- Details owns the stable `details` scope and its stable `details-play` and
  `details-back` focus IDs.
- A scope is interactive only after a valid, connected focusable has been
  registered and the engine has confirmed `activeFocusId` in that scope.
- Library virtual-focus requests are canceled before Details opens. Their
  request IDs, animation frames, and timers cannot update focus afterward.
- While a child scope is waiting for its focusable, directional and confirm
  actions are ignored instead of navigating the parent scope.
- If the requested initial focus is missing or disabled, the first valid
  focusable in the scope is used. If no valid focusable exists, the scope stays
  waiting and does not silently fall back to its parent.

## Lifecycle

Scopes track `mounting`, `waiting-for-focusable`, `activating`, `active`,
`suspended`, and `unmounting`. Registry notifications retry a pending
activation without polling. A development watchdog reports an invalid active
scope and performs one controlled recovery frame.

## DOM synchronization

Keyboard and gamepad focus is verified against `document.activeElement`. A
failed DOM focus schedules one retry on the next animation frame and then
records the failure reason without creating a retry loop.
