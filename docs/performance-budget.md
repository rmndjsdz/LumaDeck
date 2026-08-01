# Presupuesto de rendimiento

Objetivos iniciales:

- Resolución espacial habitual menor de 2 ms.
- Respuesta visual en el siguiente frame.
- Cero actualizaciones React por frame de Gamepad API.
- Sin listeners globales por pantalla.
- Sin timers por pantalla.
- Un único loop de gamepad y detenido si no hay gamepad conectado.
- Sin renders globales por cada cambio de foco.

El registry cachea `DOMRect` y comparte `ResizeObserver`. El adapter de gamepad
mantiene su estado previo de botones y ejes imperativamente. Zustand recibe
acciones discretas, foco e input mode; no recibe el estado analógico de cada
frame.

El overlay mide el tiempo de resolución y cantidad de candidatos. No debe
convertirse en una fuente de trabajo de alta frecuencia ni permanecer activo
en producción.
