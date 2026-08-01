# ADR 008: settling para navegación rápida

## Estado

Aceptado.

## Decisión

Centralizar la política en `NavigationSettlingController`, con fases
`idle`, `navigating`, `fast-navigating` y `settling`, un único timer y una
ventana de 112 ms más 128 ms de estabilización. El input y el foco lógico no
esperan a esta política.

## Consecuencias

Se cancela trabajo visual obsoleto y solo el destino final solicita el fondo.
El estado observable es pequeño y los timers no se dispersan entre tarjetas o
vistas. La ventana puede ajustarse con pruebas de interacción sin tocar las
invariantes del grid.
