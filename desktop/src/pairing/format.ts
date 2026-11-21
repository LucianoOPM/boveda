// Textos y formato de la ventana de emparejamiento (#101). Funciones puras para
// poder probarlas con `bun test` sin DOM.

import type { EndReason } from "./types";

/** Últimos segundos en que el temporizador pasa a `warning` (`DESIGN.md` §8). */
export const WARNING_MS = 20_000;

/**
 * `m:ss` de lo que falta, redondeando hacia arriba: con tiempo por delante nunca
 * muestra `0:00`. Es solo visual: el plazo real lo cuenta Rust.
 */
export function formatRemaining(ms: number): string {
  const totalSeconds = Math.max(0, Math.ceil(ms / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${String(seconds).padStart(2, "0")}`;
}

/** ¿Quedan 20 s o menos? */
export function isWarning(ms: number): boolean {
  return ms <= WARNING_MS;
}

/** El motivo por el que terminó la sesión, en español neutro. */
export function reasonText(reason: EndReason): string {
  switch (reason) {
    case "expired":
      return "El código caducó. Genera otro para emparejar.";
    case "rejected":
      return "Rechazaste el emparejamiento.";
    case "confirmationTimeout":
      return "Se agotó el tiempo para confirmar.";
    case "networkChanged":
      return "La red cambió. Genera otro código.";
    case "phoneDisconnected":
      return "El celular se desconectó. Genera otro código.";
    case "tooManyAttempts":
      return "Hubo demasiados intentos con un código incorrecto. Genera otro código.";
    case "saveFailed":
      return "No se pudo guardar el celular. Genera otro código e inténtalo de nuevo.";
    case "identityUnavailable":
      return "No se pudo leer la identidad de la PC.";
    case "codeUnavailable":
      return "No se pudo generar el código. Inténtalo de nuevo.";
  }
}
