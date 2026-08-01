# Sistema de navegación

## Flujo unificado

Los adaptadores traducen eventos físicos a `NavigationAction`. `InputManager`
es la única instancia activa y despacha la acción al `NavigationEngine`.
Home, Library y Details solo registran elementos con `Focusable` y agrupan con
`FocusScope` o layouts conductuales.

```text
mouse / keyboard / gamepad
            ↓
      InputManager
            ↓ NavigationAction
      NavigationEngine
       ↙       ↓       ↘
  Registry  Zustand  ScrollManager
       ↓
  DOM focus + callbacks
```

Zustand conserva `inputMode`, scopes/foco activos, foco anterior, última acción,
fase de navegación y métricas pequeñas. `FocusRegistry` mantiene nodos,
scope, vecinos y rectángulos cacheados de forma imperativa.

## Focus, scopes y grid

Cada `Focusable` declara un `focusId` estable, un nodo, un `scopeId` y sus
callbacks. Los scopes pueden anidarse; Details pausa el padre y restaura el
opener exacto. Un scope solo es interactivo cuando existe un focusable válido
y el engine confirma el foco DOM.

Library conserva una cuadrícula lógica absoluta de cinco columnas, 12 filas
visibles, dos filas de overscan y una ventana máxima aproximada de 60 tarjetas.
El índice es el del catálogo filtrado; la fila final es estricta y no sustituye
columnas. Un destino no montado usa `pendingFocusRequest` con índice, columna y
`requestId`; la invariantes de `virtual-grid.ts` no cambian.

## Scroll e input mode

`FocusScrollManager` usa `nearest`; durante navegación rápida evita smooth scroll
acumulado y cuando el input se estabiliza permite un ajuste suave. La rueda y
hover activan mouse; teclado/gamepad conservan el foco lógico y ocultan el
cursor tras un breve delay sin remontar componentes.

## Navegación rápida y settling

`NavigationSettlingController` clasifica el movimiento en `idle`, `navigating`,
`fast-navigating` y `settling`. Mantiene un único timer: cada movimiento
reinicia la ventana de 112 ms; al detenerse emite `settling` y vuelve a `idle`
tras 128 ms. El input y el foco lógico no esperan a esta política.

`BackgroundManager` recibe la fase y guarda solo el último destino durante
`navigating`/`fast-navigating`. Así no inicia un crossfade por cada tarjeta
atravesada. Al estabilizarse precarga/decodifica el destino final y descarta
solicitudes obsoletas mediante `requestId`.

## Transiciones de vista

`App Shell` y `BackgroundManager` permanecen montados. Home, Library y
Details se renderizan dentro de `ViewTransition`, cuya key reinicia la entrada
cuando cambia la vista; una navegación nueva reemplaza inmediatamente la
animación obsoleta. Details activa su scope y foco antes de aceptar acciones y
Back restaura `returnFocusId`.
