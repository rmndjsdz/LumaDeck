# ADR 006: Gamepad API con polling controlado

## Decisión

El gamepad usa `requestAnimationFrame` solo mientras existe un dispositivo
conectado, detecta transiciones y deja el estado analógico fuera de React.

## Motivo

La API del navegador no ofrece un stream de eventos de ejes y requiere polling;
un loop controlado evita trabajo constante cuando no hay gamepad.

## Gatillos de navegación principal

LT/L2 y RT/R2 se leen primero como los botones estándar 6 y 7, usando su
valor analógico. Cuando el mando expone esos gatillos como ejes, se usa el eje
2 para LT/L2 y el eje 5 para RT/R2.

La activación ocurre al alcanzar `0.75` y la liberación exige bajar hasta
`0.55`. Esta hysteresis evita ruido cerca del umbral y garantiza una sola
acción semántica por pulsación. Mantener el gatillo no repite la acción; el
estado se rearma únicamente al cruzar el umbral de liberación, sin timers.
