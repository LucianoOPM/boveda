// Ventana «Emparejar celular» (#101, `docs/DESIGN.md` §8).
//
// Rust es la única fuente de verdad: esta vista solo dibuja el estado que recibe
// y envía órdenes sin datos. La cuenta regresiva es solo visual; el plazo real
// y el permiso de aceptar los decide Rust. La imagen del QR contiene el código de
// un solo uso: vive solo en el estado de React y se descarta al cambiar de fase.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";

import { formatRemaining, isWarning, reasonText } from "./format";
import type { PairingView } from "./types";
import "./pairing.css";

const STATE_EVENT = "pairing://state";

/** Milisegundos que faltan, contados localmente desde el último dato de Rust. */
function useRemaining(view: PairingView | null): number | null {
  const remainingMs =
    view && (view.phase === "waiting" || view.phase === "confirming")
      ? view.remainingMs
      : null;
  const deadline = useRef(0);
  const [remaining, setRemaining] = useState<number | null>(remainingMs);

  useEffect(() => {
    if (remainingMs === null) {
      setRemaining(null);
      return;
    }
    deadline.current = performance.now() + remainingMs;
    setRemaining(remainingMs);
    const timer = window.setInterval(() => {
      setRemaining(Math.max(0, deadline.current - performance.now()));
    }, 1000);
    return () => window.clearInterval(timer);
  }, [remainingMs]);

  return remaining;
}

const command = (name: string) => () => {
  void invoke(name);
};
const cancel = command("pairing_cancel");
const regenerate = command("pairing_regenerate");
const accept = command("pairing_accept");
const reject = command("pairing_reject");

export default function PairingWindow() {
  const [view, setView] = useState<PairingView | null>(null);
  const remaining = useRemaining(view);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    // Primero se escucha y luego se pide el estado vigente: un evento anterior a
    // la carga se pierde, y `pairing_snapshot` lo recupera. Ambos salen del hilo
    // principal de Rust en orden, así que basta con aplicarlos como llegan.
    void listen<PairingView>(STATE_EVENT, (event) => {
      if (active) setView(event.payload);
    }).then((stop) => {
      if (active) unlisten = stop;
      else stop();
    });
    void invoke<PairingView>("pairing_snapshot").then((snapshot) => {
      if (active) setView(snapshot);
    });
    return () => {
      active = false;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && view?.phase !== "saving") cancel();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [view?.phase]);

  if (view === null || view.phase === "closed") {
    return <main className="pairing" aria-busy="true" />;
  }

  switch (view.phase) {
    case "noNetwork":
      return (
        <main className="pairing">
          <h1 className="pairing__title">Emparejar celular</h1>
          <p className="pairing__text" role="status">
            No se encontró una red privada. Conecta la PC a una red que Windows
            marque como privada.
          </p>
          <div className="pairing__actions">
            <button type="button" className="button" onClick={cancel}>
              Cancelar
            </button>
          </div>
        </main>
      );

    case "waiting":
      return (
        <main className="pairing">
          <h1 className="pairing__title">Emparejar celular</h1>
          <p className="pairing__text">
            Escanea este código con Bóveda en tu celular.
          </p>
          <div className="qr-card">
            <img
              className="qr"
              src={view.qr}
              alt="Código QR para emparejar el celular"
              draggable={false}
            />
          </div>
          <p
            className={`pairing__timer${
              remaining !== null && isWarning(remaining) ? " pairing__timer--warning" : ""
            }`}
          >
            CADUCA EN {formatRemaining(remaining ?? view.remainingMs)}
          </p>
          <div className="pairing__actions">
            <button type="button" className="button" onClick={cancel}>
              Cancelar
            </button>
          </div>
        </main>
      );

    case "confirming":
      return (
        <main className="pairing pairing--compact">
          <h1 className="pairing__title pairing__title--name" title={view.name}>
            <span className="name">¿Emparejar con {view.name}?</span>
          </h1>
          <p className="pairing__fingerprint" aria-label="Huella de la llave">
            {view.fingerprint}
          </p>
          <div className="pairing__actions">
            {/* «Rechazar» tiene el foco por defecto (DESIGN.md §8). */}
            <button
              type="button"
              className="button"
              onClick={reject}
              autoFocus
            >
              Rechazar
            </button>
            <button
              type="button"
              className="button button--primary"
              onClick={accept}
            >
              Emparejar
            </button>
          </div>
        </main>
      );

    case "saving":
      return (
        <main className="pairing pairing--compact">
          <p className="pairing__text" role="status">
            Guardando…
          </p>
        </main>
      );

    case "done":
      return (
        <main className="pairing pairing--compact">
          <h1 className="pairing__title pairing__title--name" title={view.name}>
            <span className="name">Emparejado con {view.name}</span>
          </h1>
          <div className="pairing__actions">
            <button
              type="button"
              className="button button--primary"
              onClick={cancel}
              autoFocus
            >
              Listo
            </button>
          </div>
        </main>
      );

    case "ended":
      return (
        <main className="pairing">
          <h1 className="pairing__title">Emparejar celular</h1>
          <div className="qr-card qr-card--faded" aria-hidden="true">
            {/* Marcador fijo: no es un QR y no contiene ningún dato. */}
            <img className="qr" src={view.placeholder} alt="" draggable={false} />
          </div>
          <p className="pairing__text" role="status">
            {reasonText(view.reason)}
          </p>
          <div className="pairing__actions">
            <button type="button" className="button" onClick={cancel}>
              Cancelar
            </button>
            <button
              type="button"
              className="button button--primary"
              onClick={regenerate}
              autoFocus
            >
              Generar otro código
            </button>
          </div>
        </main>
      );
  }
}
