# Sistema de motion

## Tokens

`src/ui/motion/motion-tokens.ts` y `src/App.css` comparten la siguiente escala:

| Token                | Duración | Uso                            |
| -------------------- | -------: | ------------------------------ |
| instant              |    88 ms | respuesta secundaria mínima    |
| focus-fast           |   150 ms | foco de teclado/gamepad        |
| standard             |   204 ms | contenido y controles          |
| panel                |   212 ms | modal y Details                |
| view-enter           |   275 ms | entrada Home/Library/Details   |
| view-exit            |   176 ms | presupuesto de salida          |
| background-crossfade |   344 ms | intercambio de fondo           |
| card-spring          |   260 ms | overshoot de selección de card |

Curvas: `standard` es `cubic-bezier(0.2, 0.8, 0.2, 1)`, `enter` es
`cubic-bezier(0.16, 1, 0.3, 1)`, `exit` es `cubic-bezier(0.4, 0, 1, 1)`,
`focus` es `cubic-bezier(0.18, 0.9, 0.25, 1)` y `spring` es
`cubic-bezier(0.34, 1.56, 0.64, 1)` (overshoot; solo se usa en `transform`,
nunca en `opacity` ni en el zoom del hero, para no revelar bordes del fondo).

El hero de Home (`home-hero-art-incoming`) reutiliza el keyframe
`home-feature-enter` con variables locales para entrar con un settle de
zoom+blur (`scale(1.045)` a `scale(1)`, `blur(6px)` a `blur(0)`) en vez de un
crossfade plano; antes esta capa tenía una regla `.is-visible` que ninguna
vista llegaba a aplicar, así que el arte nunca cruzaba en fade y el cambio se
sentía como un corte duro.

## Reglas

- El foco lógico y DOM se actualiza primero; la animación solo presenta el cambio.
- Las tarjetas usan `transform: translate3d + scale` y no alteran width, height,
  grid tracks ni medidas registradas.
- `will-change` solo aparece en el estado activo mediante el selector CSS.
- El fondo anima únicamente `opacity` y conserva la capa visible en error.
- Reduced motion usa CSS de duración mínima y el manager omite el crossfade.

`ViewTransition` conserva el App Shell y cambia la key de entrada cuando el
store solicita otra vista; una navegación nueva reemplaza inmediatamente la
animación obsoleta.
