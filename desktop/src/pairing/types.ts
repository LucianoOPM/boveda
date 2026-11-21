// Replica de `PairingView` (Rust, `desktop/src-tauri/src/pairing/view.rs`): lo
// único que el webview recibe de la sesión de emparejamiento. `qr` y
// `placeholder` son `data:` URI de SVG; el QR contiene el código de un solo uso
// y es información sensible: se guarda solo en el estado de React.

export type EndReason =
  | "expired"
  | "rejected"
  | "confirmationTimeout"
  | "networkChanged"
  | "phoneDisconnected"
  | "tooManyAttempts"
  | "saveFailed"
  | "identityUnavailable"
  | "codeUnavailable";

export const END_REASONS: readonly EndReason[] = [
  "expired",
  "rejected",
  "confirmationTimeout",
  "networkChanged",
  "phoneDisconnected",
  "tooManyAttempts",
  "saveFailed",
  "identityUnavailable",
  "codeUnavailable",
];

export type PairingView =
  | { phase: "closed" }
  | { phase: "noNetwork" }
  | { phase: "waiting"; qr: string; remainingMs: number }
  | {
      phase: "confirming";
      name: string;
      fingerprint: string;
      remainingMs: number;
    }
  | { phase: "saving"; name: string }
  | { phase: "done"; name: string }
  | { phase: "ended"; reason: EndReason; placeholder: string };
