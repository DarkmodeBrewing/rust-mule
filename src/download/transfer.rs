use crate::download::protocol::{self, Ed2kFileHash};
use crate::download::service::{BlockRange, InboundPacket};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const PROTOCOL_EDONKEY: u8 = 0x02;
pub const PROTOCOL_PACKED: u8 = 0x03;
pub const PROTOCOL_EMULE: u8 = 0x04;
pub const PACKET_HEADER_SIZE: usize = 6;
pub const MAX_TRANSFER_PACKET_SIZE: usize = 4 * 1024 * 1024 + 64;

pub type Result<T> = std::result::Result<T, TransferError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferPacket {
    pub protocol: u8,
    pub opcode: u8,
    pub payload: Vec<u8>,
}

#[derive(Debug)]
pub enum TransferError {
    Io(std::io::Error),
    InvalidPacketLength(u32),
    PacketTooLarge { limit: usize, actual: usize },
    UnsupportedProtocol(u8),
    UnexpectedOpcode(u8),
    Protocol(protocol::ProtocolError),
}

impl std::fmt::Display for TransferError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(source) => write!(f, "{source}"),
            Self::InvalidPacketLength(length) => {
                write!(f, "invalid transfer packet length: {length}")
            }
            Self::PacketTooLarge { limit, actual } => {
                write!(f, "transfer packet too large: {actual} > {limit}")
            }
            Self::UnsupportedProtocol(protocol) => {
                write!(f, "unsupported transfer protocol 0x{protocol:02x}")
            }
            Self::UnexpectedOpcode(opcode) => {
                write!(f, "unexpected transfer opcode 0x{opcode:02x}")
            }
            Self::Protocol(source) => write!(f, "{source}"),
        }
    }
}

impl std::error::Error for TransferError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(source) => Some(source),
            Self::Protocol(source) => Some(source),
            Self::InvalidPacketLength(_)
            | Self::PacketTooLarge { .. }
            | Self::UnsupportedProtocol(_)
            | Self::UnexpectedOpcode(_) => None,
        }
    }
}

impl From<std::io::Error> for TransferError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<protocol::ProtocolError> for TransferError {
    fn from(value: protocol::ProtocolError) -> Self {
        Self::Protocol(value)
    }
}

impl TransferPacket {
    pub fn new(protocol: u8, opcode: u8, payload: Vec<u8>) -> Result<Self> {
        ensure_supported_protocol(protocol)?;
        ensure_packet_size(payload.len())?;
        Ok(Self {
            protocol,
            opcode,
            payload,
        })
    }

    pub fn requestparts(file_hash: Ed2kFileHash, blocks: &[BlockRange]) -> Result<Self> {
        let payload = protocol::encode_requestparts_payload(file_hash, blocks)?;
        Self::new(PROTOCOL_EDONKEY, protocol::OP_REQUESTPARTS, payload)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        ensure_supported_protocol(self.protocol)?;
        ensure_packet_size(self.payload.len())?;

        let packet_len =
            self.payload
                .len()
                .checked_add(1)
                .ok_or(TransferError::PacketTooLarge {
                    limit: MAX_TRANSFER_PACKET_SIZE,
                    actual: usize::MAX,
                })?;
        let mut out = Vec::with_capacity(PACKET_HEADER_SIZE + self.payload.len());
        out.push(self.protocol);
        out.extend_from_slice(&(packet_len as u32).to_le_bytes());
        out.push(self.opcode);
        out.extend_from_slice(&self.payload);
        Ok(out)
    }

    pub fn decode(frame: &[u8]) -> Result<Self> {
        if frame.len() < PACKET_HEADER_SIZE {
            return Err(TransferError::InvalidPacketLength(frame.len() as u32));
        }
        let protocol = frame[0];
        ensure_supported_protocol(protocol)?;
        let packet_len = u32::from_le_bytes([frame[1], frame[2], frame[3], frame[4]]);
        if packet_len == 0 {
            return Err(TransferError::InvalidPacketLength(packet_len));
        }
        let payload_len =
            usize::try_from(packet_len - 1).map_err(|_| TransferError::PacketTooLarge {
                limit: MAX_TRANSFER_PACKET_SIZE,
                actual: usize::MAX,
            })?;
        ensure_packet_size(payload_len)?;
        let expected = PACKET_HEADER_SIZE + payload_len;
        if frame.len() != expected {
            return Err(TransferError::InvalidPacketLength(packet_len));
        }
        Ok(Self {
            protocol,
            opcode: frame[5],
            payload: frame[6..].to_vec(),
        })
    }
}

pub async fn read_packet<R>(reader: &mut R) -> Result<TransferPacket>
where
    R: AsyncRead + Unpin,
{
    let mut header = [0u8; PACKET_HEADER_SIZE];
    reader.read_exact(&mut header).await?;
    ensure_supported_protocol(header[0])?;
    let packet_len = u32::from_le_bytes([header[1], header[2], header[3], header[4]]);
    if packet_len == 0 {
        return Err(TransferError::InvalidPacketLength(packet_len));
    }
    let payload_len =
        usize::try_from(packet_len - 1).map_err(|_| TransferError::PacketTooLarge {
            limit: MAX_TRANSFER_PACKET_SIZE,
            actual: usize::MAX,
        })?;
    ensure_packet_size(payload_len)?;
    let mut payload = vec![0u8; payload_len];
    reader.read_exact(&mut payload).await?;
    Ok(TransferPacket {
        protocol: header[0],
        opcode: header[5],
        payload,
    })
}

pub async fn write_packet<W>(writer: &mut W, packet: &TransferPacket) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    let frame = packet.encode()?;
    writer.write_all(&frame).await?;
    writer.flush().await?;
    Ok(())
}

pub async fn request_block<S>(
    stream: &mut S,
    file_hash: Ed2kFileHash,
    block: BlockRange,
) -> Result<InboundPacket>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let request = TransferPacket::requestparts(file_hash, &[block])?;
    write_packet(stream, &request).await?;

    let response = read_packet(stream).await?;
    match response.opcode {
        protocol::OP_SENDINGPART | protocol::OP_COMPRESSEDPART => Ok(InboundPacket {
            opcode: response.opcode,
            payload: response.payload,
        }),
        other => Err(TransferError::UnexpectedOpcode(other)),
    }
}

fn ensure_supported_protocol(protocol: u8) -> Result<()> {
    match protocol {
        PROTOCOL_EDONKEY | PROTOCOL_PACKED | PROTOCOL_EMULE => Ok(()),
        other => Err(TransferError::UnsupportedProtocol(other)),
    }
}

fn ensure_packet_size(payload_len: usize) -> Result<()> {
    if payload_len > MAX_TRANSFER_PACKET_SIZE {
        return Err(TransferError::PacketTooLarge {
            limit: MAX_TRANSFER_PACKET_SIZE,
            actual: payload_len,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_TRANSFER_PACKET_SIZE, PACKET_HEADER_SIZE, PROTOCOL_EDONKEY, TransferError,
        TransferPacket, read_packet, request_block, write_packet,
    };
    use crate::download::protocol::{self, OP_REQUESTPARTS};
    use crate::download::service::BlockRange;

    #[test]
    fn requestparts_packet_encodes_edonkey_header() {
        let packet = TransferPacket::requestparts([0x11; 16], &[BlockRange { start: 0, end: 15 }])
            .expect("requestparts packet");
        let frame = packet.encode().expect("encode");

        assert_eq!(frame[0], PROTOCOL_EDONKEY);
        assert_eq!(frame[5], OP_REQUESTPARTS);
        assert_eq!(
            u32::from_le_bytes([frame[1], frame[2], frame[3], frame[4]]) as usize,
            packet.payload.len() + 1
        );
        assert_eq!(frame.len(), PACKET_HEADER_SIZE + packet.payload.len());
    }

    #[test]
    fn decode_rejects_zero_length_packets() {
        let frame = [PROTOCOL_EDONKEY, 0, 0, 0, 0, OP_REQUESTPARTS];
        let err = TransferPacket::decode(&frame).expect_err("zero length must fail");
        assert!(matches!(err, TransferError::InvalidPacketLength(0)));
    }

    #[test]
    fn decode_rejects_unsupported_protocol() {
        let frame = [0x99, 1, 0, 0, 0, OP_REQUESTPARTS];
        let err = TransferPacket::decode(&frame).expect_err("unsupported protocol");
        assert!(matches!(err, TransferError::UnsupportedProtocol(0x99)));
    }

    #[test]
    fn new_rejects_oversized_payloads() {
        let err = TransferPacket::new(
            PROTOCOL_EDONKEY,
            OP_REQUESTPARTS,
            vec![0u8; MAX_TRANSFER_PACKET_SIZE + 1],
        )
        .expect_err("oversized packet");
        assert!(matches!(
            err,
            TransferError::PacketTooLarge {
                limit: MAX_TRANSFER_PACKET_SIZE,
                actual
            } if actual == MAX_TRANSFER_PACKET_SIZE + 1
        ));
    }

    #[tokio::test]
    async fn read_write_packet_roundtrip() {
        let packet = TransferPacket::requestparts(
            [0x22; 16],
            &[
                BlockRange { start: 0, end: 15 },
                BlockRange { start: 32, end: 47 },
            ],
        )
        .expect("requestparts packet");
        let (mut client, mut server) = tokio::io::duplex(1024);
        let expected = packet.clone();

        let writer = tokio::spawn(async move {
            write_packet(&mut client, &packet)
                .await
                .expect("write packet");
        });
        let read = read_packet(&mut server).await.expect("read packet");
        writer.await.expect("writer join");

        assert_eq!(read, expected);
    }

    #[tokio::test]
    async fn request_block_returns_sendingpart_payload() {
        let requested = BlockRange { start: 0, end: 3 };
        let sending_payload = protocol::encode_sendingpart_payload(
            [0x44; 16],
            requested.start,
            requested.end + 1,
            &[1, 2, 3, 4],
        )
        .expect("sendingpart payload");
        let response =
            TransferPacket::new(PROTOCOL_EDONKEY, protocol::OP_SENDINGPART, sending_payload)
                .expect("response packet");
        let (mut client, mut server) = tokio::io::duplex(1024);

        let responder = tokio::spawn(async move {
            let request = read_packet(&mut server).await.expect("read request");
            assert_eq!(request.opcode, OP_REQUESTPARTS);
            write_packet(&mut server, &response)
                .await
                .expect("write response");
        });

        let inbound = request_block(&mut client, [0x44; 16], requested)
            .await
            .expect("request block");
        responder.await.expect("responder join");

        assert_eq!(inbound.opcode, protocol::OP_SENDINGPART);
    }

    #[tokio::test]
    async fn request_block_rejects_unexpected_opcode() {
        let response = TransferPacket::new(PROTOCOL_EDONKEY, 0x01, vec![]).expect("response");
        let (mut client, mut server) = tokio::io::duplex(1024);

        let responder = tokio::spawn(async move {
            let _ = read_packet(&mut server).await.expect("read request");
            write_packet(&mut server, &response)
                .await
                .expect("write response");
        });

        let err = request_block(&mut client, [0x55; 16], BlockRange { start: 8, end: 15 })
            .await
            .expect_err("unexpected opcode must fail");
        responder.await.expect("responder join");

        assert!(matches!(err, TransferError::UnexpectedOpcode(0x01)));
    }
}
