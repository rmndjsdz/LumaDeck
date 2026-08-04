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

## Screen Adapter V1

Screen Adapter V1 es una capa declarativa opt-in entre una pantalla y las APIs
de navegación existentes. Home es la primera pantalla migrada; Library y
Details continúan usando temporalmente el flujo legacy para permitir una
migración incremental sin cambiar su comportamiento.

La regla de ownership es explícita:

```text
Pantalla declara estructura e intención.
Screen Adapter traduce la declaración.
ProductShell controla la route.
Navigation Engine controla contexto, resolución, restore y foco.
FocusRegistry controla disponibilidad.
```

El `NavigationScreenDefinition` V1 soporta únicamente:

- identidad de pantalla y route;
- root scope y parent scope opcional;
- foco inicial;
- regiones y relaciones padre/hijo;
- grupos de filas e índices lógicos declarados por la pantalla;
- política de restauración existente;
- activación opt-in.

`ScreenNavigationAdapter` traduce esa definición hacia `FocusScope` y la
notificación de route activa del `NavigationEngine`. No mantiene una copia
autoritativa de `NavigationContext`, no calcula vecinos, no resuelve
direcciones, no accede a coordenadas DOM y no crea generaciones, restores ni
transacciones propias. El Engine sigue siendo el dueño de contexto,
resolución, restauración, foco, sincronización DOM y trazas; `FocusRegistry`
solo informa qué focusables están disponibles.

En Home, la definición declara `product-shell`, `main-nav-home`, la región
`home-content` y el grupo `home-rows`. Home continúa construyendo y ordenando
los datos, filtrando juegos, asignando `rowId`, `rowIndex` e `itemIndex`, y
abriendo Details mediante la acción existente.

ProductShell conserva temporalmente el control de la route, la coordinación
de navegación principal, la preparación de apertura de Details y la solicitud
de restauración al cerrar Details. Library permanece sin migrar y Details
continúa siendo un scope legacy compatible con el adapter de Home. El campo
`ProductStore.returnFocusId` también permanece temporalmente para mantener la
compatibilidad del flujo actual.

Queda fuera de V1:

- una abstracción genérica de virtualización;
- un framework declarativo de acciones;
- un framework completo de transiciones;
- la migración de Details o Library;
- la eliminación de `returnFocusId`;
- la eliminación de las APIs actuales del Engine;
- la obligación de usar el contrato en todas las pantallas.

Una futura pantalla puede migrarse cuando su estructura pueda expresarse con
el contrato V1, sus callbacks de dominio permanezcan fuera del adaptador y la
paridad de foco, restore, navegación direccional, mouse y gamepad pueda
demostrarse con pruebas y validación manual. La migración debe seguir siendo
opt-in y convivir con las pantallas legacy hasta que exista evidencia de
paridad.

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

## Jerarquía Home / Library

La barra principal es el nivel `main-navigation`. Cada vista de contenido
declara un único nivel hijo (`home-content` o `library-content`) mediante
`navigationRegion`; el engine resuelve la entrada y salida, sin que Home ni
Library calculen vecinos globales.

`ArrowDown` desde una pestaña entra al foco configurado o al último foco de su
contenido. `ArrowUp` desde el primer nivel de Home o desde la primera fila de
Library vuelve a la pestaña correspondiente. Cambiar de pestaña no entra
automáticamente al contenido: la vista se monta y la siguiente acción Down
realiza la entrada explícita. El coordinador conserva el último `focusId` y,
para Library, el índice lógico necesario para materializar una tarjeta fuera
de la ventana visible.

Los ids de la barra son estables: `main-nav-home` y `main-nav-library`. El
overlay de desarrollo muestra nivel, padre/hijo, transición, restauración y
el último foco por región. La restauración desde Details conserva prioridad y
solo usa `returnFocusId` cuando el opener pertenece a la vista de retorno.

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

## Continuidad del contexto de navegacion

### Contexto canonico

`NavigationContext` pertenece al `NavigationEngine`. Los componentes y las
pantallas pueden solicitar acciones, registrar focusables y declarar scopes,
pero no mantienen copias autoritativas del contexto. `activeFocusId`,
`itemIndex`, `preferredItemIndex`, region, fila y generacion deben permanecer
coherentes dentro del Engine.

### Memoria de fila ligada a generacion

La memoria de una fila pertenece a una generacion. El Engine rechaza
explicitamente memoria stale de una generacion anterior antes de resolver una
navegacion vertical; una memoria de la generacion actual puede conservar una
posicion previamente visitada cuando sigue siendo valida.

### Restauracion de ruta

Una restauracion explicita pertenece a una transicion semantica e idempotente.
Debe producir exactamente un `CONTEXT_RESTORE_COMMIT`. Los ciclos de montaje y
desmontaje, incluido `React.StrictMode`, no cambian el resultado logico.
`scope-unregister` y el registro de focusables solo informan lifecycle y
disponibilidad; no crean restauraciones competidoras ni generaciones paralelas.

Si el target aun no esta materializado, el restore queda pendiente y espera al
registro valido sin ejecutar un fallback prematuro. No se usan timeouts
arbitrarios como mecanismo de sincronizacion.

### Invariante de continuidad

La navegacion despues de volver de una vista secundaria debe ser equivalente a
la navegacion sin abandonar el scope:

```text
navigate(context, direction)
===
navigate(restore(save(context)), direction)
```

### Convergencia de input

Mouse, teclado y gamepad son fuentes distintas de entrada, pero convergen en
el mismo contexto logico antes de ejecutar la resolucion direccional. No hay
algoritmos de navegacion separados por dispositivo.

## Estados de entrega

Las tareas de navegacion pueden pasar por estos estados:

```text
IMPLEMENTED
AUTOMATED VERIFICATION PASSED
READY FOR PRODUCT QA
ACCEPTED
```

`ACCEPTED` requiere la validacion funcional del Product Owner; no se deriva
unicamente de unit tests, integracion, build o lint.

Screen Adapter V1 y la migración de Home alcanzaron ese estado después de la
validación manual en la aplicación real, incluida la paridad con gamepad.

## Transiciones de vista

`App Shell` y `BackgroundManager` permanecen montados. Home, Library y
Details se renderizan dentro de `ViewTransition`, cuya key reinicia la entrada
cuando cambia la vista; una navegación nueva reemplaza inmediatamente la
animación obsoleta. Details activa su scope y foco antes de aceptar acciones y
Back restaura `returnFocusId`.
