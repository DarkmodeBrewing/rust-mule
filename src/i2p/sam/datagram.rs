use crate::i2p::sam::SamError;
use crate::i2p::sam::protocol::SamCommand;
use std::net::{IpAddr, SocketAddr};
use tokio::net::UdpSocket;

type Result<T> = std::result::Result<T, SamError>;

/// A SAM repliable datagram session (STYLE=DATAGRAM) with UDP forwarding enabled.
///
/// You create the session over the SAM TCP control channel (`SESSION CREATE ... STYLE=DATAGRAM`),
/// then send datagrams to the SAM UDP port (7655 by default). Incoming datagrams are forwarded
/// to a UDP port on localhost.
pub struct SamDatagramSocket {
    session_id: String,
    sam_udp: SocketAddr,
    udp: UdpSocket,
    sam_version: &'static str,
}

#[derive(Debug, Clone)]
pub struct SamDatagramRecv {
    pub from_destination: String,
    pub from_port: Option<u16>,
    pub to_port: Option<u16>,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, Default)]
pub struct SamDatagramSendOpts {
    pub from_port: Option<u16>,
    pub to_port: Option<u16>,
}

impl SamDatagramSocket {
    /// Bind a UDP socket locally and prepare to use it as the "forwarding" target.
    ///
    /// You still must create the SAM session separately on a `SamClient` (TCP),
    /// passing the returned `forward_port()` in `SESSION CREATE ... PORT=<port>`.
    pub async fn bind_for_forwarding(
        session_id: impl Into<String>,
        sam_host: IpAddr,
        sam_udp_port: u16,
        bind_ip: IpAddr,
        bind_port: u16,
    ) -> Result<Self> {
        let udp = UdpSocket::bind(SocketAddr::new(bind_ip, bind_port))
            .await
            .map_err(|e| SamError::io("bind UDP socket for SAM forwarding", e))?;

        let sam_udp = SocketAddr::new(sam_host, sam_udp_port);

        Ok(Self {
            session_id: session_id.into(),
            sam_udp,
            udp,
            // For maximum compatibility, keep sending `3.0` in datagram headers.
            // As of SAM 3.2, any `3.x` is allowed, but older servers required `3.0`.
            sam_version: "3.0",
        })
    }

    pub fn forward_port(&self) -> u16 {
        self.udp
            .local_addr()
            .expect("UDP socket must have local addr")
            .port()
    }

    pub fn forward_addr(&self) -> SocketAddr {
        self.udp
            .local_addr()
            .expect("UDP socket must have local addr")
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn sam_udp_addr(&self) -> SocketAddr {
        self.sam_udp
    }

    /// Send a repliable datagram to a remote I2P destination via SAM's UDP port.
    ///
    /// The remote must be a base64 Destination (or, on Java I2P, may also be a hostname or b32).
    pub async fn send_to(
        &self,
        destination: &str,
        payload: &[u8],
        opts: SamDatagramSendOpts,
    ) -> Result<()> {
        let mut header = String::new();
        header.push_str(self.sam_version);
        header.push(' ');
        header.push_str(&self.session_id);
        header.push(' ');
        header.push_str(destination);

        if let Some(p) = opts.from_port {
            header.push(' ');
            header.push_str("FROM_PORT=");
            header.push_str(&p.to_string());
        }
        if let Some(p) = opts.to_port {
            header.push(' ');
            header.push_str("TO_PORT=");
            header.push_str(&p.to_string());
        }

        header.push('\n');

        let mut buf = Vec::with_capacity(header.len() + payload.len());
        buf.extend_from_slice(header.as_bytes());
        buf.extend_from_slice(payload);

        self.udp
            .send_to(&buf, self.sam_udp)
            .await
            .map_err(|e| SamError::io("send UDP datagram to SAM", e))?;
        Ok(())
    }

    /// Receive an incoming forwarded repliable datagram.
    ///
    /// Forwarded repliable datagrams are prefixed with:
    /// `$destination FROM_PORT=nnn TO_PORT=nnn \n $payload`
    pub async fn recv(&self) -> Result<SamDatagramRecv> {
        let mut buf = vec![0u8; 64 * 1024];
        let (n, _from) = self
            .udp
            .recv_from(&mut buf)
            .await
            .map_err(|e| SamError::io("receive UDP datagram from SAM", e))?;
        buf.truncate(n);
        parse_forwarded_repliable(&buf)
    }
}

pub fn build_session_create_datagram_forward(
    session_id: &str,
    destination_privkey: &str,
    forward_port: u16,
    forward_host: IpAddr,
    opts: impl IntoIterator<Item = impl AsRef<str>>,
) -> Result<SamCommand> {
    // SESSION CREATE STYLE=DATAGRAM requires PORT for forwarding.
    let mut cmd = SamCommand::new("SESSION CREATE")
        .arg("STYLE", "DATAGRAM")
        .arg("ID", session_id)
        .arg("DESTINATION", destination_privkey)
        .arg("PORT", forward_port.to_string())
        .arg("HOST", forward_host.to_string());

    for opt in opts {
        let opt = opt.as_ref();
        let (k, v) = opt.split_once('=').ok_or_else(|| {
            SamError::protocol(format!(
                "SAM session option must be key=value (got '{opt}')"
            ))
        })?;
        cmd = cmd.arg(k, v);
    }

    Ok(cmd)
}

fn parse_forwarded_repliable(buf: &[u8]) -> Result<SamDatagramRecv> {
    let newline = buf
        .iter()
        .position(|b| *b == b'\n')
        .ok_or_else(|| SamError::BadFrame {
            what: "forwarded datagram missing '\\n' header delimiter",
            raw: format!("len={}", buf.len()),
        })?;

    let header = std::str::from_utf8(&buf[..newline]).map_err(|e| SamError::BadFrame {
        what: "forwarded datagram header not UTF-8",
        raw: e.to_string(),
    })?;
    let payload = buf[(newline + 1)..].to_vec();

    let mut parts = header.split_whitespace();
    let from_destination = parts
        .next()
        .ok_or_else(|| SamError::BadFrame {
            what: "forwarded datagram missing destination header",
            raw: header.to_string(),
        })?
        .to_string();

    let mut from_port = None;
    let mut to_port = None;
    for p in parts {
        if let Some(v) = p.strip_prefix("FROM_PORT=") {
            from_port = Some(v.parse::<u16>().map_err(|e| SamError::BadFrame {
                what: "bad FROM_PORT value",
                raw: e.to_string(),
            })?);
        } else if let Some(v) = p.strip_prefix("TO_PORT=") {
            to_port = Some(v.parse::<u16>().map_err(|e| SamError::BadFrame {
                what: "bad TO_PORT value",
                raw: e.to_string(),
            })?);
        } else {
            // Be permissive; we may add support for Datagram3 headers later.
        }
    }

    if from_destination.is_empty() {
        return Err(SamError::BadFrame {
            what: "forwarded datagram destination header is empty",
            raw: header.to_string(),
        });
    }

    Ok(SamDatagramRecv {
        from_destination,
        from_port,
        to_port,
        payload,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn parses_forwarded_repliable() {
        let pkt = b"destbase64 FROM_PORT=1 TO_PORT=2\nhello";
        let r = parse_forwarded_repliable(pkt).unwrap();
        assert_eq!(r.from_destination, "destbase64");
        assert_eq!(r.from_port, Some(1));
        assert_eq!(r.to_port, Some(2));
        assert_eq!(r.payload, b"hello");
    }

    #[test]
    fn builds_session_create_command() {
        let cmd = build_session_create_datagram_forward(
            "sess",
            "priv",
            9999,
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            ["inbound.quantity=2", "outbound.quantity=2"],
        )
        .unwrap();
        let line = cmd.to_line();
        assert!(line.contains("STYLE=DATAGRAM"));
        assert!(line.contains("ID=sess"));
        assert!(line.contains("DESTINATION=priv"));
        assert!(line.contains("PORT=9999"));
        assert!(line.contains("HOST=127.0.0.1"));
        assert!(line.contains("inbound.quantity=2"));
        assert!(line.contains("outbound.quantity=2"));
    }
}
