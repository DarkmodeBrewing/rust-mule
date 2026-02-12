use crate::i2p::sam::datagram_tcp::SamDatagramTcp;
use crate::i2p::sam::{SamDatagramRecv, SamDatagramSendOpts, SamDatagramSocket, SamError};

type Result<T> = std::result::Result<T, SamError>;

/// Minimal abstraction used by KAD code: "send a datagram" and "receive a datagram".
///
/// We support both UDP forwarding (SAM UDP port + local UDP bind) and iMule-style
/// TCP datagrams (`DATAGRAM SEND`/`DATAGRAM RECEIVED` on the SAM TCP socket).
pub enum SamKadSocket {
    UdpForward(SamDatagramSocket),
    Tcp(SamDatagramTcp),
}

impl SamKadSocket {
    pub fn transport_name(&self) -> &'static str {
        match self {
            Self::UdpForward(_) => "udp_forward",
            Self::Tcp(_) => "tcp",
        }
    }

    pub async fn send_to(&mut self, destination: &str, payload: &[u8]) -> Result<()> {
        match self {
            Self::UdpForward(sock) => {
                sock.send_to(destination, payload, SamDatagramSendOpts::default())
                    .await
            }
            Self::Tcp(sock) => sock.send_to(destination, payload).await,
        }
    }

    pub async fn recv(&mut self) -> Result<SamDatagramRecv> {
        match self {
            Self::UdpForward(sock) => sock.recv().await,
            Self::Tcp(sock) => sock.recv().await,
        }
    }
}
