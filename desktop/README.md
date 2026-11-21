# Bóveda para Windows

App de bandeja (Tauri 2) que recibe contraseñas del celular y las deja en el portapapeles. Las reglas y los comandos están en [`CLAUDE.md`](CLAUDE.md); la arquitectura, en [`../ARCHITECTURE.md`](../ARCHITECTURE.md).

## Red y Firewall

El servidor escucha y se anuncia por mDNS **solo** en las direcciones IPv4 de redes que Windows marca como **privadas**. Revisa el perfil de red cada 5 s: si ninguna red es privada (o no puede saberlo), cierra los sockets y retira el anuncio. Limitación de `mdns-sd`: mientras haya al menos una red privada activa existen sockets UDP 5353 comodín que descartan sin responder lo que llega por las demás interfaces (ver la especificación).

El Firewall es una segunda capa. El instalador (NSIS, instalación por usuario) crea dos reglas de entrada limitadas al perfil **Privado**: TCP para la app y UDP 5353 (mDNS) para la app. Pide permiso de administrador (UAC) solo para ese comando; si se rechaza, la app funciona igual, pero las reglas no existen. Al desinstalar se borran.

### Desarrollo (`bun run tauri dev`)

`tauri dev` **no** crea las reglas. Si Windows pregunta por el acceso, o para que el celular alcance la PC en una red privada, créalas a mano desde una consola con permisos de administrador (ajusta la ruta del ejecutable de desarrollo):

```bat
netsh advfirewall firewall add rule name="Boveda servidor (dev)" dir=in action=allow program="E:\proyectos\boveda\target\debug\boveda-desktop.exe" profile=private protocol=TCP enable=yes
netsh advfirewall firewall add rule name="Boveda mDNS (dev)" dir=in action=allow program="E:\proyectos\boveda\target\debug\boveda-desktop.exe" profile=private protocol=UDP localport=5353 enable=yes
```

Para quitarlas:

```bat
netsh advfirewall firewall delete rule name="Boveda servidor (dev)"
netsh advfirewall firewall delete rule name="Boveda mDNS (dev)"
```

Las pruebas de la red real y de mDNS (`#[ignore]`) se ejecutan con `cargo test -p boveda-desktop --test network_windows --test mdns_interfaces -- --ignored --nocapture --test-threads=1`.
