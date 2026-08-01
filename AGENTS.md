# LumaDeck — guía de trabajo

## Alcance actual

La navegación es una plataforma compartida. Las features declaran
`focusId`, `scopeId`, agrupaciones, foco inicial y callbacks; no registran
listeners globales ni calculan vecinos por su cuenta.

## Reglas

- Mantener `package-lock.json` y utilizar npm.
- Mantener TypeScript strict y evitar `any`, `@ts-ignore` y casts usados para
  ocultar errores.
- El estado observable mínimo vive en Zustand. El registro de focusables,
  mediciones y loops viven en servicios imperativos.
- Todo listener, `requestAnimationFrame`, `ResizeObserver` o timer debe tener
  cleanup.
- Validar con `npm run format:check`, `npm run typecheck`, `npm run lint`,
  `npm run test`, `npm run build` y `cargo check`.
- No añadir integraciones reales de biblioteca, Steam o lanzamiento hasta que
  la plataforma de navegación esté estable.

## Estructura

- `src/features/catalog`: catálogo local determinista y consulta TanStack Query.
- `src/features/launcher`: shell persistente y vistas Home, Library y Details.
- `src/ui/background`, `src/ui/input` y `src/ui/performance`: servicios
  transversales de fondo, cursor, diagnóstico de gamepad y métricas.

- `src/ui/navigation/core`: acciones, registry, engine y navegación espacial.
- `src/ui/navigation/input`: adaptadores físicos y repetición.
- `src/ui/navigation/focus`: `Focusable`, `FocusScope` y contexto.
- `src/ui/navigation/layouts`: layouts conductuales sin identidad visual fija.
- `src/features/navigation-demo`: demostración técnica navegable.
- `docs/`: decisiones, presupuesto y pruebas del sistema.

## Motion y rendimiento perceptivo

- Las duraciones visuales usan los tokens de `src/ui/motion/motion-tokens.ts`
  y sus variables CSS equivalentes; no se añaden duraciones aisladas en
  componentes.
- `NavigationSettlingController` es el único dueño de la política de
  navegación rápida, settling y su timer.
- `BackgroundManager` conserva como máximo dos capas visibles y una caché LRU
  pequeña; nunca bloquea el foco ni descarga recursos externos.
- HUD y performance marks solo se activan en desarrollo; el HUD requiere
  `?hud=1` o `VITE_PERFORMANCE_HUD=true`.
- Todo timer, RAF, observer y listener añadido debe cancelarse en cleanup.
