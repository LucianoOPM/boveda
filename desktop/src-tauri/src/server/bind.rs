//! Enlaza todas las IPv4 privadas de una generación al mismo puerto (#100).

use std::collections::BTreeSet;
use std::io::{self, ErrorKind};
use std::net::{Ipv4Addr, TcpListener};

/// Listeners de una generación: uno por IPv4 privada, todos en el mismo puerto
/// y en modo no bloqueante.
#[derive(Debug)]
pub(crate) struct Bound {
    pub(crate) listeners: Vec<TcpListener>,
    pub(crate) port: u16,
}

/// Por qué no se pudo enlazar. Sin direcciones: solo el tipo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BindError {
    /// No hay ninguna dirección que enlazar.
    NoAddresses,
    /// Error de E/S distinto de «puerto ocupado».
    Io(ErrorKind),
    /// Se agotaron los intentos de encontrar un puerto común.
    NoPort,
}

/// Enlaza **todas** las `ips` al mismo puerto o a ninguno.
///
/// Primero el `preferred`. Si está ocupado (o reservado por Windows), se obtiene
/// un puerto candidato enlazando una vez a la primera IP con el puerto 0 y se
/// intentan enlazar todas las IPs a ese puerto, con el sondeo ya soltado. Si una
/// carrera hace fallar alguna, se cierra todo y se repite con un candidato
/// nuevo, hasta `attempts` veces. Nunca se hace `bind(ip:0)` por cada IP: Windows
/// podría asignar puertos distintos.
pub(crate) fn bind_all(
    ips: &BTreeSet<Ipv4Addr>,
    preferred: u16,
    attempts: u32,
) -> Result<Bound, BindError> {
    let first = *ips.iter().next().ok_or(BindError::NoAddresses)?;
    match bind_on_port(ips, preferred) {
        Ok(bound) => return Ok(bound),
        Err(error) if !port_unavailable(&error) => return Err(BindError::Io(error.kind())),
        Err(_) => {}
    }
    for _ in 0..attempts {
        let candidate = TcpListener::bind((first, 0))
            .and_then(|probe| probe.local_addr())
            .map_err(|error| BindError::Io(error.kind()))?
            .port();
        // El sondeo ya se soltó al terminar la expresión anterior.
        match bind_on_port(ips, candidate) {
            Ok(bound) => return Ok(bound),
            Err(error) if !port_unavailable(&error) => return Err(BindError::Io(error.kind())),
            Err(_) => {}
        }
    }
    Err(BindError::NoPort)
}

/// Ocupado por otro proceso o reservado (Windows reserva rangos con
/// `WSAEACCES`): vale la pena probar con otro puerto.
fn port_unavailable(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        ErrorKind::AddrInUse | ErrorKind::PermissionDenied
    )
}

/// Enlaza todas las IPs a `port`. Si una falla, las ya enlazadas se sueltan al
/// salir y no queda nada abierto.
fn bind_on_port(ips: &BTreeSet<Ipv4Addr>, port: u16) -> io::Result<Bound> {
    let mut listeners = Vec::with_capacity(ips.len());
    for ip in ips {
        let listener = TcpListener::bind((*ip, port))?;
        listener.set_nonblocking(true)?;
        listeners.push(listener);
    }
    Ok(Bound { listeners, port })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::net::{Ipv4Addr, TcpListener};

    use super::{BindError, bind_all};

    fn ips(list: &[Ipv4Addr]) -> BTreeSet<Ipv4Addr> {
        list.iter().copied().collect()
    }

    const LOCAL_1: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 1);
    const LOCAL_2: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 2);

    /// Un puerto libre en este momento.
    fn free_port() -> u16 {
        TcpListener::bind((LOCAL_1, 0))
            .expect("bind")
            .local_addr()
            .expect("addr")
            .port()
    }

    #[test]
    fn uses_preferred_port_when_free() {
        let preferred = free_port();
        let bound = bind_all(&ips(&[LOCAL_1, LOCAL_2]), preferred, 5).expect("bind_all");
        assert_eq!(bound.port, preferred);
        assert_eq!(bound.listeners.len(), 2);
        for listener in &bound.listeners {
            assert_eq!(listener.local_addr().expect("addr").port(), preferred);
        }
    }

    #[test]
    fn falls_back_to_one_shared_port_when_preferred_is_busy() {
        let busy = TcpListener::bind((LOCAL_1, 0)).expect("bind");
        let preferred = busy.local_addr().expect("addr").port();
        let bound = bind_all(&ips(&[LOCAL_1, LOCAL_2]), preferred, 5).expect("bind_all");
        assert_ne!(bound.port, preferred);
        assert_eq!(bound.listeners.len(), 2);
        for listener in &bound.listeners {
            assert_eq!(listener.local_addr().expect("addr").port(), bound.port);
        }
    }

    #[test]
    fn fails_if_any_ip_cannot_bind_and_leaves_nothing_open() {
        // 203.0.113.1 (TEST-NET-3) no está asignada a esta máquina.
        let unassigned = Ipv4Addr::new(203, 0, 113, 1);
        let preferred = free_port();
        let result = bind_all(&ips(&[LOCAL_1, unassigned]), preferred, 5);
        assert!(matches!(result, Err(BindError::Io(_))), "{result:?}");
        // Nada quedó abierto: el puerto preferido sigue libre.
        TcpListener::bind((LOCAL_1, preferred)).expect("el puerto debe estar libre");
    }

    #[test]
    fn empty_set_is_error() {
        assert_eq!(
            bind_all(&BTreeSet::new(), 47615, 5).map(|_| ()),
            Err(BindError::NoAddresses)
        );
    }

    #[test]
    fn no_port_when_no_attempts_are_left() {
        let busy = TcpListener::bind((LOCAL_1, 0)).expect("bind");
        let preferred = busy.local_addr().expect("addr").port();
        assert_eq!(
            bind_all(&ips(&[LOCAL_1]), preferred, 0).map(|_| ()),
            Err(BindError::NoPort)
        );
    }

    #[test]
    fn listeners_do_not_block() {
        let bound = bind_all(&ips(&[LOCAL_1]), free_port(), 5).expect("bind_all");
        let error = bound.listeners[0].accept().expect_err("sin clientes");
        assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
    }
}
