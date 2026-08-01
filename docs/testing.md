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

## Comandos

```bash
npm run format:check
npm run typecheck
npm run lint
npm run test
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
npm run tauri:build
```

La validación manual cubre mouse, flechas, WASD, repetición sostenida,
Enter/Space, Escape, disabled, grid, scroll, cambio de input mode, gamepad,
reduced motion y consola limpia. Para revisar rendimiento se usa `?hud=1`
(compacto) o `?hud=detail` (detallado); sin query el HUD no aparece.
