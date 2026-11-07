# desktop/ (app Windows, Tauri 2)

App de bandeja que recibe contraseñas del celular y las deja en el portapapeles. Usa `vault-core` directamente para el protocolo y toda la criptografía.

## Comandos (desde desktop/)

- Gestor de paquetes: bun. No uses npm, yarn ni pnpm ni agregues sus lockfiles.
- Instalar dependencias del frontend: `bun install`
- Desarrollo: `bun run tauri dev`
- Compilación: `bun run tauri build`

## Reglas

- Las contraseñas nunca cruzan al webview: no pases secretos por el IPC de Tauri. La interfaz solo muestra el QR y la lista de dispositivos.
- Portapapeles: escribe con `arboard` usando sus opciones de Windows para excluir el contenido del historial de Win+V y de la sincronización en la nube.
- A los 30 s, borra el portapapeles solo si todavía contiene la misma contraseña. Para comprobarlo, guarda un hash, no la contraseña.
- El servidor escucha solo en redes marcadas como privadas y rechaza durante el intercambio Noise cualquier dispositivo no emparejado, antes de leer contenido.
- La llave privada de la PC se guarda protegida con DPAPI; nunca en texto plano en disco.
- La app vive en la bandeja con cuatro opciones: emparejar celular, dispositivos emparejados, iniciar con Windows y salir. No agregues ventanas fuera de esas.
