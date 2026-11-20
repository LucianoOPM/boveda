# Huella tras importar un respaldo (#86)

Fecha: 2026-11-19. Diseño corto acordado en la conversación. Corrige el flujo de la #44 (`2026-11-14-respaldos-selector-design.md`, punto 6 de "Tras importar con éxito").

## Intención

Un usuario con la huella activa que importa un respaldo no debe tener que volver a decidir si la activa. Hoy `runRestore` desactiva la huella (la copia del Keystore protegía la llave de la bóveda anterior y ya no abre nada) y `VaultGate` ofrece activarla como si fuera la primera vez. Se pierde una decisión que el usuario ya tomó.

## Metas

- Si la huella **estaba activa antes de importar**, al terminar la importación la app la **vuelve a activar** con la llave de la bóveda importada, sin mostrar la pantalla de oferta. El usuario ve una sola verificación de huella (la de siempre al activarla) y llega a la bóveda.
- Si la huella **no estaba activa**, nada cambia: se ofrece como hoy (`_afterCreate`).
- Si el usuario cancela la huella o la activación falla, se cae en lo de hoy: la pantalla de oferta, desde donde puede reintentar u omitir. Nunca se queda atrapado ni sin bóveda.
- Solo Ajustes reactiva la huella (`replacesVault`). En el primer uso no hay bóveda anterior y una copia vieja del Keystore no representa nada que conservar: ahí se ofrece como hoy.

## No metas

- Tocar la criptografía ni el núcleo. Se reutiliza `enableBiometric` tal cual (`beginEnable`, `exportVaultKey`, `completeEnable`): la llave sale del núcleo solo después de la huella, como al activarla desde cero.
- Dejar la copia vieja del Keystore: se sigue borrando con `biometric.disable()` en el mismo punto de hoy, antes de activar la nueva.
- Fusionar respaldos o la clave de recuperación (#88 y #89).
- Cambiar los textos ni el diseño de la pantalla de oferta.

## Anti-metas

- Que la huella quede activa con una copia de la llave anterior (el desbloqueo con huella fallaría con `WrongVaultKey`).
- Que la llave de la bóveda cruce a Dart sin una huella aceptada.
- Que cancelar la huella bloquee la bóveda o cierre la sesión.
- Que un bloqueo durante la activación deje la bóveda desbloqueada o monte una pantalla sobre el desbloqueo (la comprobación de `_isAway` y `_enterUnlocked` de hoy se conserva).

## Enfoques

- **A. Reactivar sola** (elegido, recomendado): `runRestore` anota si la huella estaba activa antes de importar y lo devuelve en `RestoreImported`; `VaultGate._onImported` activa la huella si estaba activa y, si no sale, muestra la oferta.
- **B. Avisar:** mantener la oferta con un texto que explique que importar desactivó la huella. Es más chico, pero sigue pidiendo una acción ya decidida.
- **C. Dejarlo:** como hoy.

## Decisiones de diseño

- `RestoreImported` gana `reenableBiometric` (bool, `false` por omisión). `runRestore` lo calcula con `biometric.isEnabled()` **antes** de importar y solo con `replacesVault` (con `try/catch`, `false` si falla) y lo entrega junto al resultado.
- `onImported` de `SettingsScreen` y `onRestored` de `FirstRunScreen` pasan a recibir también ese indicador (`reenableBiometric`, nominal). `VaultGate._onImported` lo recibe y, si es `true` y el dispositivo sigue admitiéndola, llama a `enableBiometric`; si devuelve `enabled`, entra directo a la bóveda; con `canceled` o `failed` sigue por `_afterCreate` (la oferta).
- El resto de `_onImported` (quitar lo apilado, subir `_epoch`, pasar por `checking`) no cambia.

## Lo que queda al implementador

Nombres exactos de los parámetros, el orden de los commits y qué pruebas existentes se ajustan por el cambio de firma de los callbacks (se anotan en el PR). Pruebas nuevas, en rojo primero: importar con la huella activa la reactiva sin oferta; con la huella cancelada cae en la oferta; con la huella inactiva ofrece como hoy; con un bloqueo durante la activación no se monta nada sobre el desbloqueo; y el primer uso no reactiva aunque exista una copia vieja. Verificación final: `flutter test` y `flutter analyze` en `mobile/`, `cargo test --workspace` y `cargo clippy` en la raíz. La prueba en el teléfono la haces tú.
