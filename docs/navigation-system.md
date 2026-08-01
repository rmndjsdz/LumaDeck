# Sistema de navegación

## Flujo unificado

Los adaptadores traducen eventos físicos a `NavigationAction`. El
`InputManager` es la única instancia activa y despacha la acción al
`NavigationEngine`. Las pantallas solo registran elementos con `Focusable` y
agrupan elementos con `FocusScope` o los layouts conductuales.

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

## Acciones y estado

`NavigationAction` contiene `move-up`, `move-down`, `move-left`, `move-right`,
`confirm`, `back`, `menu`, `page-next` y `page-previous`. `InputMode` distingue
`mouse`, `keyboard` y `gamepad`.

Zustand conserva únicamente `inputMode`, scope/foco activos, foco anterior,
última acción y métricas pequeñas. `FocusRegistry` mantiene de forma
imperativa los nodos, sus rectángulos cacheados y callbacks.

## Focus Registry

Cada entrada necesita un `focusId` estable, un nodo, un `scopeId` y puede
declarar vecinos explícitos, disabled, hidden, prioridad y callbacks. El
registry registra/desregistra automáticamente, observa resize mediante un
observer compartido, invalida medidas al cambiar el layout e ignora nodos
desconectados.

## Focusable

```tsx
<Focusable
  focusId="demo-item-1"
  scopeId="demo-grid"
  disabled={false}
  onConfirm={() => openDetails()}
>
  Item
</Focusable>
```

El componente administra `tabIndex`, `data-focus-id`, `aria-disabled`, hover,
click, foco lógico y sincronización con el input mode. El contenido no necesita
saber si la acción vino de mouse, teclado o gamepad.

## FocusScope

```tsx
<FocusScope
  scopeId="demo-library"
  initialFocusId="demo-item-0"
  restoreFocus
  rememberScroll
  trapFocus={false}
>
  {children}
</FocusScope>
```

Los scopes pueden anidarse. Un scope modal se prepara con el foco que lo abrió,
pausa el scope padre y al desmontarse restaura ese foco exacto.

## Motor espacial

`NavigationTabs` y `NavigationRow` declaran grupos lineales horizontales. El
engine resuelve Left/Right por índice estable y solo aplica wrap cuando el
layout lo solicita; no dependen de la geometría de los rectángulos. Los grids
pueden declarar `index` e `itemCount` lógicos para soportar ventanas virtuales:
si el destino todavía no está montado, el layout lo materializa y el foco se
aplica después del commit.

`findSpatialCandidate` es una función pura: filtra por dirección, prioriza
alineación y solapamiento de ejes, combina distancia principal y perpendicular,
y rompe empates por prioridad y `focusId`. El engine primero prueba overrides,
luego candidatos válidos del scope, mueve el foco DOM y pide visibilidad al
`FocusScrollManager`.

## Inputs

- Teclado: flechas/WASD, Enter/Space, Escape/Backspace y PageUp/PageDown.
- Mouse: movimiento acumulado con umbral, hover, click y wheel.
- Gamepad: D-pad, stick izquierdo, botones 0/1 y bumpers 4/5, deadzone y
  transiciones.

La repetición direccional es un servicio compartido con retraso inicial de
260 ms, intervalo de 90 ms y aceleración opcional a 58 ms. Se cancela al
soltar, cambiar de dirección, perder foco o detener el manager.

## Scroll y debug

La aplicación de producto reutiliza el mismo scope persistente para header,
footer y contenido. Los scopes modales se registran como exclusivos: el scope
padre queda pausado, sus candidatos no participan y `trapFocus` intercepta
Tab y navegación direccional hasta el cierre. Al cerrar, se restaura el opener
y el scroll recordado del scope anterior.

`FocusScrollManager` solo desplaza cuando el elemento no es visible y usa
`scrollIntoView({ block: "nearest", inline: "nearest" })`. `ScrollRestoration`
recuerda `scrollTop`/`scrollLeft` por scope. El overlay de desarrollo expone
modo, scope, foco, candidatos, tiempo espacial, gamepad, restauración y
métricas ligeras.
