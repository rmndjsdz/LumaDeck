# ADR 006: Gamepad API con polling controlado

## Decisión

El gamepad usa `requestAnimationFrame` solo mientras existe un dispositivo
conectado, detecta transiciones y deja el estado analógico fuera de React.

## Motivo

La API del navegador no ofrece un stream de eventos de ejes y requiere polling;
un loop controlado evita trabajo constante cuando no hay gamepad.
