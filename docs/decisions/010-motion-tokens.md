# ADR 010: tokens únicos de motion

## Estado

Aceptado.

## Decisión

Las duraciones y curvas visuales viven en `motion-tokens.ts` y variables CSS
equivalentes. Foco, paneles, vistas, Details y fondos consumen esa escala;
reduced motion la colapsa en CSS y en el manager.

## Consecuencias

La experiencia conserva ritmo consistente y los cambios de identidad visual
no introducen números arbitrarios en componentes. Los delays de contenido son
variables derivadas de `instant` y tokens de stagger.
