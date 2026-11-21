// La app vive en la bandeja. Cada ventana se crea desde Rust con su vista en el
// parámetro `view`: la de emparejar celular (#101); la de dispositivos
// emparejados llega con #107.
import PairingWindow from "./pairing/PairingWindow";

function App() {
  const view = new URLSearchParams(window.location.search).get("view");
  if (view === "pairing") return <PairingWindow />;
  return null;
}

export default App;
