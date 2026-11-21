//! Backend real del detector: Administrador de listas de redes de Windows (#100).

use std::net::Ipv4Addr;

use windows::Win32::Networking::NetworkListManager::{
    INetworkListManager, NLM_ENUM_NETWORK_CONNECTED, NLM_NETWORK_CATEGORY_PRIVATE,
    NetworkListManager,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::core::GUID;
use windows_sys::Win32::Foundation::ERROR_BUFFER_OVERFLOW;
use windows_sys::Win32::NetworkManagement::IpHelper::{
    GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST, GetAdaptersAddresses,
    GetIfEntry2, IF_TYPE_ETHERNET_CSMACD, IF_TYPE_IEEE80211, IP_ADAPTER_ADDRESSES_LH, MIB_IF_ROW2,
};
use windows_sys::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows_sys::Win32::Networking::WinSock::{AF_INET, SOCKADDR_IN};

use super::{Candidate, NetworkProbe, ProbeError, is_usable};

/// Tope de elementos al recorrer las enumeraciones de COM y la lista de
/// adaptadores: nadie tiene tantas redes, y evita un bucle sin fin si una
/// enumeración no termina.
const MAX_ITEMS: usize = 512;

/// Intentos de `GetAdaptersAddresses` si el tamaño cambia entre llamadas.
const ADAPTER_ATTEMPTS: usize = 4;

/// Detector real: una red es privada si Windows le asignó la categoría
/// `Private`. `DomainAuthenticated` y `Public` no cuentan.
///
/// COM se inicializa (MTA) en la **primera** llamada a `private_addresses`, en
/// el hilo que la hace, y se libera al soltar la sonda: hay que usarla y
/// soltarla en el mismo hilo (el supervisor la mueve a su hilo antes de usarla).
#[derive(Debug)]
pub struct NlmProbe {
    com_initialized: bool,
}

impl NlmProbe {
    pub fn new() -> Self {
        Self {
            com_initialized: false,
        }
    }

    fn ensure_com(&mut self) -> Result<(), ProbeError> {
        if self.com_initialized {
            return Ok(());
        }
        // SAFETY: sin punteros; la inicialización exitosa (incluido «ya
        // inicializado», `S_FALSE`) se empareja con `CoUninitialize` en `Drop`.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if hr.is_err() {
            // Por ejemplo `RPC_E_CHANGED_MODE`: el hilo ya es de otro tipo.
            return Err(ProbeError);
        }
        self.com_initialized = true;
        Ok(())
    }
}

impl Default for NlmProbe {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for NlmProbe {
    fn drop(&mut self) {
        if self.com_initialized {
            // SAFETY: empareja el `CoInitializeEx` exitoso de `ensure_com`, en
            // el mismo hilo.
            unsafe { CoUninitialize() };
        }
    }
}

impl NetworkProbe for NlmProbe {
    fn private_candidates(&mut self) -> Result<Vec<Candidate>, ProbeError> {
        self.ensure_com()?;
        let private_ids = private_adapter_ids()?;
        let adapters = adapter_addresses()?;
        let mut candidates = Vec::new();
        for id in &private_ids {
            // Una red privada cuyo adaptador no aparece es una duda: se rechaza
            // todo (falla cerrando).
            let adapter = adapters
                .iter()
                .find(|adapter| adapter.name.eq_ignore_ascii_case(id))
                .ok_or(ProbeError)?;
            // Solo direcciones a las que un celular pueda llegar (ver `normalize`).
            candidates.extend(adapter.ips.iter().filter(|ip| is_usable(**ip)).map(|ip| {
                Candidate {
                    ip: *ip,
                    eligible: adapter.eligible,
                    has_gateway: adapter.has_gateway,
                    metric: adapter.metric,
                }
            }));
        }
        Ok(candidates)
    }
}

/// Un adaptador con las IPv4 que tiene y lo que hace falta para elegirlo (#101).
struct Adapter {
    name: String,
    ips: Vec<Ipv4Addr>,
    eligible: bool,
    has_gateway: bool,
    metric: u32,
}

/// ¿Puede ir su dirección en el QR? Debe estar activo, ser de hardware (no un
/// adaptador virtual de Hyper-V, WSL, VMware o VirtualBox, ni un túnel o VPN) y
/// de tipo Ethernet o Wi-Fi. Se decide por el tipo y el indicador de hardware de
/// Windows, no por el nombre del adaptador, que es frágil.
fn is_eligible_adapter(if_type: u32, oper_up: bool, hardware: bool) -> bool {
    oper_up && hardware && matches!(if_type, IF_TYPE_ETHERNET_CSMACD | IF_TYPE_IEEE80211)
}

/// ¿Es un adaptador de hardware según `GetIfEntry2`? Cualquier fallo es «no»
/// (falla cerrando: un adaptador dudoso no va en el QR).
///
/// # Safety
/// `adapter` debe ser un elemento válido de la lista de `GetAdaptersAddresses`.
unsafe fn is_hardware(adapter: &IP_ADAPTER_ADDRESSES_LH) -> bool {
    // SAFETY: `MIB_IF_ROW2` es un conjunto de enteros y arreglos: todo ceros es
    // un valor válido. La API solo lee `InterfaceLuid` y rellena el resto.
    let mut row: MIB_IF_ROW2 = unsafe { std::mem::zeroed() };
    row.InterfaceLuid = adapter.Luid;
    // SAFETY: `row` es propia y está inicializada; `GetIfEntry2` escribe en ella.
    if unsafe { GetIfEntry2(&mut row) } != 0 {
        return false;
    }
    // El primer bit de `InterfaceAndOperStatusFlags` es `HardwareInterface`.
    row.InterfaceAndOperStatusFlags._bitfield & 1 != 0
}

/// `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}` en mayúsculas: el formato de
/// `AdapterName` en `GetAdaptersAddresses`.
fn guid_string(guid: &GUID) -> String {
    format!(
        "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        guid.data1,
        guid.data2,
        guid.data3,
        guid.data4[0],
        guid.data4[1],
        guid.data4[2],
        guid.data4[3],
        guid.data4[4],
        guid.data4[5],
        guid.data4[6],
        guid.data4[7],
    )
}

/// Ids de adaptador de las conexiones de las redes conectadas con categoría
/// `Private`.
fn private_adapter_ids() -> Result<Vec<String>, ProbeError> {
    // SAFETY: llamadas COM sobre interfaces propias, con COM ya inicializado en
    // este hilo; los punteros de salida los gestiona `windows`.
    unsafe {
        let manager: INetworkListManager =
            CoCreateInstance(&NetworkListManager, None, CLSCTX_ALL).map_err(|_| ProbeError)?;
        let networks = manager
            .GetNetworks(NLM_ENUM_NETWORK_CONNECTED)
            .map_err(|_| ProbeError)?;
        let mut ids = Vec::new();
        for _ in 0..MAX_ITEMS {
            let mut slot = [None];
            let mut fetched = 0u32;
            networks
                .Next(&mut slot, Some(&mut fetched))
                .map_err(|_| ProbeError)?;
            let Some(network) = slot[0].take().filter(|_| fetched == 1) else {
                return Ok(ids);
            };
            if network.GetCategory().map_err(|_| ProbeError)? != NLM_NETWORK_CATEGORY_PRIVATE {
                continue;
            }
            let connections = network.GetNetworkConnections().map_err(|_| ProbeError)?;
            for _ in 0..MAX_ITEMS {
                let mut slot = [None];
                let mut fetched = 0u32;
                connections
                    .Next(&mut slot, Some(&mut fetched))
                    .map_err(|_| ProbeError)?;
                let Some(connection) = slot[0].take().filter(|_| fetched == 1) else {
                    break;
                };
                let adapter = connection.GetAdapterId().map_err(|_| ProbeError)?;
                ids.push(guid_string(&adapter));
            }
        }
        // Más elementos de los posibles: es una duda.
        Err(ProbeError)
    }
}

/// `AdapterName` y direcciones IPv4 unicast de cada adaptador.
fn adapter_addresses() -> Result<Vec<Adapter>, ProbeError> {
    let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
    let mut size: u32 = 16 * 1024;
    for _ in 0..ADAPTER_ATTEMPTS {
        // `u64` para que el búfer quede alineado a 8 bytes, como pide la estructura.
        let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
        let first = buffer.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
        // SAFETY: `first` apunta a un búfer propio de al menos `size` bytes y
        // `size` es el tamaño que se informa a la API.
        let status = unsafe {
            GetAdaptersAddresses(
                u32::from(AF_INET),
                flags,
                std::ptr::null(),
                first,
                &mut size,
            )
        };
        if status == ERROR_BUFFER_OVERFLOW {
            continue; // `size` ya trae el tamaño necesario.
        }
        if status != 0 {
            return Err(ProbeError);
        }
        // SAFETY: tras un éxito el búfer contiene una lista enlazada válida que
        // vive mientras `buffer` siga en pie (hasta el final de esta iteración).
        return unsafe { read_adapters(first) };
    }
    Err(ProbeError)
}

/// # Safety
/// `first` debe ser el inicio de una lista que devolvió `GetAdaptersAddresses`
/// y su búfer debe seguir vivo durante la llamada.
unsafe fn read_adapters(first: *const IP_ADAPTER_ADDRESSES_LH) -> Result<Vec<Adapter>, ProbeError> {
    let mut adapters = Vec::new();
    let mut adapter = first;
    for _ in 0..MAX_ITEMS {
        if adapter.is_null() {
            return Ok(adapters);
        }
        // SAFETY: no es nulo y pertenece a la lista válida (ver arriba).
        let current = unsafe { &*adapter };
        if current.AdapterName.is_null() {
            return Err(ProbeError);
        }
        // SAFETY: `AdapterName` es una cadena ANSI terminada en cero de la API.
        let name = unsafe { std::ffi::CStr::from_ptr(current.AdapterName.cast()) }
            .to_string_lossy()
            .into_owned();
        let mut ips = Vec::new();
        let mut unicast = current.FirstUnicastAddress;
        for _ in 0..MAX_ITEMS {
            if unicast.is_null() {
                break;
            }
            // SAFETY: elemento válido de la lista de direcciones del adaptador.
            let entry = unsafe { &*unicast };
            let sockaddr = entry.Address.lpSockaddr;
            if !sockaddr.is_null() {
                // SAFETY: la API se pidió con `AF_INET`, así que cada dirección
                // es un `SOCKADDR_IN`; se comprueba la familia antes de usarla.
                let v4 = unsafe { &*sockaddr.cast::<SOCKADDR_IN>() };
                if v4.sin_family == AF_INET {
                    // SAFETY: `S_un` es una unión de enteros; `S_addr` es válido.
                    let octets = unsafe { v4.sin_addr.S_un.S_addr }.to_ne_bytes();
                    ips.push(Ipv4Addr::from(octets));
                }
            }
            unicast = entry.Next;
        }
        let eligible = is_eligible_adapter(
            current.IfType,
            current.OperStatus == IfOperStatusUp,
            // SAFETY: `current` es un elemento válido de la lista (ver arriba).
            unsafe { is_hardware(current) },
        );
        adapters.push(Adapter {
            name,
            ips,
            eligible,
            has_gateway: !current.FirstGatewayAddress.is_null(),
            metric: current.Ipv4Metric,
        });
        adapter = current.Next;
    }
    Err(ProbeError)
}

#[cfg(test)]
mod tests {
    use windows::core::GUID;

    use super::{guid_string, is_eligible_adapter};

    const ETHERNET: u32 = 6;
    const WIFI: u32 = 71;
    const LOOPBACK: u32 = 24;
    const TUNNEL: u32 = 131;

    #[test]
    fn physical_ethernet_and_wifi_that_are_up_are_eligible() {
        assert!(is_eligible_adapter(ETHERNET, true, true));
        assert!(is_eligible_adapter(WIFI, true, true));
    }

    #[test]
    fn virtual_adapters_are_not_eligible_even_if_up_and_ethernet() {
        // Hyper-V, WSL, VMware o VirtualBox se ven como Ethernet pero no son hardware.
        assert!(!is_eligible_adapter(ETHERNET, true, false));
    }

    #[test]
    fn adapters_that_are_down_are_not_eligible() {
        assert!(!is_eligible_adapter(ETHERNET, false, true));
        assert!(!is_eligible_adapter(WIFI, false, true));
    }

    #[test]
    fn loopback_and_tunnels_are_not_eligible() {
        assert!(!is_eligible_adapter(LOOPBACK, true, true));
        assert!(!is_eligible_adapter(TUNNEL, true, true));
    }

    #[test]
    fn guid_string_matches_the_adapter_name_format() {
        let guid = GUID::from_u128(0xdcb00c01_570f_4a9b_8d69_199fdba5723b);
        assert_eq!(guid_string(&guid), "{DCB00C01-570F-4A9B-8D69-199FDBA5723B}");
    }
}
