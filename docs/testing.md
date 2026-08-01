# Pruebas

La suite usa Vitest con `jsdom`.

## Unitarias

- mapa de teclado y exclusión de campos editables;
- repetición, aceleración y cancelación;
- deadzone y transición direccional de gamepad;
- registro, desregistro y focusId duplicado;
- navegación espacial, overrides, empates y disabled;
- confirmación y restauración de scope/modal.

## Integración

`navigation-demo.integration.test.tsx` monta la aplicación de producto real,
comprueba el shell persistente, la ventana de Library, filtros, apertura/cierre
de Details y la restauración del foco.

`navigation-demo.integration.test.tsx` monta la aplicación real, comprueba que
la demo registra focusables y que existe un foco activo desde el arranque.

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

La validación manual debe cubrir mouse, flechas, WASD, repetición sostenida,
Enter/Space, modal/Escape, disabled, tabs, grid, scroll, cambio de input mode,
Gamepad API y consola limpia.
