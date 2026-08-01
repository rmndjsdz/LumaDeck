# ADR 009: BackgroundManager y caché acotada

## Estado

Aceptado.

## Decisión

`BackgroundManager` mantiene solo las capas current/incoming y una caché LRU
imperativa de seis recursos. Cada URL comparte una promesa pendiente; cada
solicitud visual tiene `requestId`, conserva el fondo anterior en error y
crossfadea únicamente opacity después de load/decode.

## Consecuencias

No se duplican precargas ni se acumulan capas grandes. La caché cubre actual,
anterior, siguiente y vecinos inmediatos sin crecer ilimitadamente. Los SVG
siguen siendo locales y simulados.
