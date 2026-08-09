# Bluetooth V1

## Decisión

Bluetooth se integra como una capacidad de System Settings con tres capas:

`BluetoothView` → `bluetooth-service.ts` → comandos Tauri → `bluetooth.rs`.

La UI solo recibe el modelo estable `BluetoothSnapshot`. Cada operación nativa
devuelve una nueva consulta del estado de Windows; no se persisten dispositivos,
claves, PIN ni datos de bonding en LumaDeck.

## APIs de Windows

En Windows se usan las APIs WinRT de la crate `windows`:

- `Windows.Devices.Radios.Radio` para consultar y cambiar el radio Bluetooth.
- `Windows.Devices.Enumeration.DeviceInformation` para enumerar dispositivos
  emparejados, olvidar dispositivos y ejecutar pairing.
- `DeviceWatcher` con los selectores de `BluetoothDevice` y
  `BluetoothLEDevice` para discovery Classic/BLE durante el flujo activo.
- `BluetoothDevice`/`BluetoothLEDevice` para consultar conexión y
  `BluetoothClassOfDevice` para clasificar cuando Windows lo expone.

## Capacidades V1 realmente soportadas

- consultar radios Bluetooth, disponibilidad y estado real;
- activar y desactivar el radio;
- enumerar dispositivos emparejados y reflejar conexión;
- iniciar/detener discovery real, sin polling permanente en background;
- emparejar mediante el procedimiento estándar de Windows;
- olvidar mediante `DeviceInformationPairing.UnpairAsync`;
- navegación con los focus IDs existentes y deduplicación por identidad estable.

## Limitaciones conocidas

Windows no ofrece una operación genérica y fiable para conectar o desconectar
manualmente todos los perfiles Bluetooth desde WinRT. Por eso V1 no muestra
acciones de conexión/desconexión que no podamos garantizar. La reconexión
automática queda bajo control de Windows.

El pairing usa `PairAsync`, por lo que los flujos con PIN, confirmación o
entrada física pueden presentar el procedimiento estándar de Windows. V2 puede
añadir `CustomPairing` y un estado de ceremonia dentro de LumaDeck cuando se
defina un contrato asíncrono para confirmación/PIN sin bloquear comandos.

La batería, RSSI y `lastSeen` permanecen vacíos cuando Windows no los expone de
forma fiable. La validación con hardware físico sigue pendiente: este entorno
permite compilar y probar la lógica, pero no tiene un accesorio Bluetooth
disponible para ejecutar el flujo de pairing extremo a extremo.
