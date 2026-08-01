# ADR 001: acciones de input unificadas

## Decisión

Mouse, teclado y gamepad traducen entradas físicas a `NavigationAction` antes
de llegar al engine.

## Motivo

Las vistas no deben conocer teclas, botones ni ejes. Esto permite cambiar un
adaptador o simularlo en pruebas sin modificar la navegación de una pantalla.
