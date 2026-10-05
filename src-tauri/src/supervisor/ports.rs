//! Port availability (R4) and the addresses friends join with (R15).

use std::net::{Ipv4Addr, TcpListener, UdpSocket};

/// True when nothing else is listening on the port. The test listener is closed
/// immediately.
pub fn port_available(port: u16) -> bool {
    TcpListener::bind((Ipv4Addr::UNSPECIFIED, port)).is_ok()
}

/// The PC's address on the local network. Connecting a UDP socket sends no packets;
/// it only asks Windows which interface would route to the internet.
pub fn lan_ip() -> Option<String> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect(("8.8.8.8", 80)).ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then(|| ip.to_string())
}
