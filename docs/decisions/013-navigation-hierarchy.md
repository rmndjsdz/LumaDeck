# Decisión 013: jerarquía explícita entre tabs y contenido

## Contexto

Home y Library comparten el shell, pero sus bordes de navegación no son
vecinos espaciales. El primer card de Home debía volver a Home y la primera
fila de Library debía volver a Library; además, una pestaña debía poder entrar
al último card conocido sin secuestrar el foco al cambiar de vista.

## Decisión

El `NavigationEngine` usa `NavigationLevelCoordinator`, un servicio
imperativo y puro en cuanto a decisión, con regiones declarativas por
`focusable`:

- `main-navigation` declara `childRegionId` y su `entryFocusId`.
- `home-content` y `library-content` declaran `parentRegionId` y
  `exitFocusId`.
- El coordinador guarda solo ids e índices lógicos: nunca nodos DOM ni
  listeners globales.
- `Down` entra al hijo y `Up` sale por el parent/exit explícito. Home usa el
  coordinador de filas; Library conserva su grid absoluto y su materialización
  existente.
- Los cambios de pestaña no enfocan contenido automáticamente. La entrada se
  produce con una acción Down; la única excepción controlada es restaurar el
  `returnFocusId` de Details al volver a la vista opener.

## Consecuencias

La navegación transversal queda centralizada y las features solo declaran
identidad, región, foco inicial y callbacks. Los ids `main-nav-home` y
`main-nav-library` son parte del contrato de integración. El overlay de
desarrollo expone el nivel activo y las razones de transición/restauración.
Las pruebas puras cubren selección de entrada, salida explícita y memoria de
índice; las pruebas de integración cubren Home, Library y la restauración
desde Details.
