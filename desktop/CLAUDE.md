# desktop/ (app Windows, Tauri 2)

App de bandeja que recibe contraseñas del celular y las deja en el portapapeles. Usa `vault-core` directamente para el protocolo y toda la criptografía.

Antes de crear o cambiar una ventana o un estilo, lee `docs/DESIGN.md` (secciones 2 a 4, 5.2 y 8).

## Comandos (desde desktop/)

- Gestor de paquetes: bun. No uses npm, yarn ni pnpm ni agregues sus lockfiles.
- Instalar dependencias del frontend: `bun install`
- Desarrollo: `bun run tauri dev`
- Compilación: `bun run tauri build`

## Reglas

- Las contraseñas nunca cruzan al webview: no pases secretos por el IPC de Tauri. La interfaz solo muestra la imagen del QR (que dibuja Rust) y la lista de dispositivos; el código de un solo uso, la oferta y las llaves nunca cruzan al webview como datos, y la imagen del QR es información sensible (ni logs, ni archivos, ni mensajes de error).
- Portapapeles: escribe con `arboard` usando sus opciones de Windows para excluir el contenido del historial de Win+V y de la sincronización en la nube.
- A los 30 s, borra el portapapeles solo si todavía contiene la misma contraseña. Para comprobarlo, guarda un hash, no la contraseña.
- El canal de las contraseñas es TCP ligado únicamente a las IPv4 de interfaces cuyo perfil de Windows es `Private` (nunca `0.0.0.0`) y rechaza durante el intercambio Noise cualquier dispositivo no emparejado, antes de leer contenido. Sin una red privada apta no hay listener TCP ni daemon mDNS.
- Limitación conocida de `mdns-sd` 0.22: mientras haya al menos una red privada activa existen sockets UDP 5353 comodín (`0.0.0.0` y `[::]`), aunque las demás redes sean públicas. Bóveda deshabilita todas las interfaces del daemon y habilita solo las IPv4 privadas; por las demás, los datagramas se descartan sin responder. No escribas ni prometas «ningún socket en redes no privadas».
- La llave privada de la PC se guarda protegida con DPAPI; nunca en texto plano en disco.
- La app vive en la bandeja con cuatro opciones: emparejar celular, dispositivos emparejados, iniciar con Windows y salir. No agregues ventanas fuera de esas; la confirmación del emparejamiento es parte de la ventana de emparejar.
