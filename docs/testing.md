# Pruebas

La suite usa Vitest con `jsdom`.

## Unitarias

- mapa de teclado y exclusión de campos editables;
- repetición, aceleración y cancelación;
- deadzone y transición direccional de gamepad;
- registro, desregistro y focusId duplicado;
- navegación espacial, overrides, empates y disabled;
- confirmación y restauración de scope/modal;
- tokens de motion y curvas centralizadas;
- fases `navigating`, `fast-navigating`, `settling` y cleanup del timer;
- BackgroundManager con destino final, caché, doble capa y conservación en error;
- foco/cierre de Details y restauración del elemento exacto.

## Integración

`navigation-demo.integration.test.tsx` monta la aplicación real y comprueba el
shell persistente, la ventana de Library, filtros, apertura/cierre de Details,
restauración de foco y ciclos repetidos.

## Cobertura aceptada de Screen Adapter V1

`screen-navigation-adapter.test.tsx` protege el contrato opt-in y comprueba:

- registro y activación de la definición;
- foco inicial como fallback del Engine;
- idempotencia bajo `React.StrictMode`;
- restore exacto del contexto;
- un único `CONTEXT_RESTORE_COMMIT`;
- desmontaje sin scope activo huérfano;
- convivencia con un scope legacy de Details.

La integración de Home conserva además la cobertura de:

- Home → Details → Back;
- Up y Down inmediatamente después de Back;
- columnas 1–4 y posición lógica de fila/columna;
- mouse y gamepad;
- salida hacia `main-navigation`;
- restauración de la tarjeta exacta y foco activo no nulo.

La validación manual aceptada por el Product Owner confirmó el foco inicial,
la navegación entre filas, la restauración exacta, Up/Down tras Back, las
columnas, la salida a navegación principal, mouse, gamepad y la ausencia de
regresiones visibles en Library, Details, UI y estilos.

## Comandos

```bash
npm run format:check
npm run typecheck
npm run lint
npm run test:navigation-smoke
npm run test
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
npm run tauri:build
```

`npm run test:navigation-smoke` ejecuta la seleccion estable de regresiones
criticas del Navigation Engine, la integracion real de Home/Details, la
memoria de filas y el adaptador de gamepad. No crea infraestructura E2E
adicional y no sustituye la validacion funcional manual del Product Owner.

La validación manual cubre mouse, flechas, WASD, repetición sostenida,
Enter/Space, Escape, disabled, grid, scroll, cambio de input mode, gamepad,
reduced motion y consola limpia. Para revisar rendimiento se usa `?hud=1`
(compacto) o `?hud=detail` (detallado); sin query el HUD no aparece.
