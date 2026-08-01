# LumaDeck

Base del proyecto de escritorio LumaDeck con Tauri 2, React, TypeScript y Vite.
La demo actual valida el primer sistema unificado de navegación.

## Desarrollo

```bash
npm install
npm run dev
```

Para abrir la aplicación Tauri durante el desarrollo:

```bash
npm run tauri:dev
```

La demo muestra navegación espacial, scopes, modal, scroll, repetición de
direcciones y adaptadores de mouse, teclado y Gamepad API. No contiene todavía
biblioteca, Steam ni lanzamiento de juegos.

## Validaciones

```bash
npm run typecheck
npm run lint
npm run test
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
npm run tauri:build
```
