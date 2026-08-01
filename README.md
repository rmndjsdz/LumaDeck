# LumaDeck

Base del proyecto de escritorio LumaDeck con Tauri 2, React, TypeScript y Vite.
La demo actual valida el primer sistema unificado de navegación.

## Estado actual

La primera slice de producto incluye una shell persistente con Home, Library y
Details, un catálogo local determinista de 200 juegos y navegación por mouse,
teclado y Gamepad API. No hay integración con Steam ni lanzamiento real de
procesos.

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
