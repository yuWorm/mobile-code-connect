use mobilecode_connect_protocol::{ControlFrame, DataStreamHeader, ProtocolError};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const HEADER_LEN_SIZE: usize = 4;
const MAX_HEADER_LEN: usize = 64 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum TunnelStreamError {
    #[error("io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    #[error("json failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("header too large: {size} bytes")]
    HeaderTooLarge { size: usize },
}

pub async fn write_data_header<W>(
    writer: &mut W,
    header: &DataStreamHeader,
) -> Result<(), TunnelStreamError>
where
    W: AsyncWrite + Unpin,
{
    let bytes = header.encode_with_len_prefix()?;
    writer.write_all(&bytes).await?;
    Ok(())
}

pub async fn read_data_header<R>(reader: &mut R) -> Result<DataStreamHeader, TunnelStreamError>
where
    R: AsyncRead + Unpin,
{
    let mut prefix = [0_u8; HEADER_LEN_SIZE];
    reader.read_exact(&mut prefix).await?;

    let header_len = u32::from_be_bytes(prefix) as usize;
    if header_len > MAX_HEADER_LEN {
        return Err(TunnelStreamError::HeaderTooLarge { size: header_len });
    }

    let mut bytes = Vec::with_capacity(HEADER_LEN_SIZE + header_len);
    bytes.extend_from_slice(&prefix);
    bytes.resize(HEADER_LEN_SIZE + header_len, 0);
    reader.read_exact(&mut bytes[HEADER_LEN_SIZE..]).await?;

    Ok(DataStreamHeader::decode_with_len_prefix(&bytes)?)
}

pub async fn write_control_frame<W>(
    writer: &mut W,
    frame: &ControlFrame,
) -> Result<(), TunnelStreamError>
where
    W: AsyncWrite + Unpin,
{
    let payload = serde_json::to_vec(frame)?;
    if payload.len() > MAX_HEADER_LEN {
        return Err(TunnelStreamError::HeaderTooLarge {
            size: payload.len(),
        });
    }

    writer
        .write_all(&(payload.len() as u32).to_be_bytes())
        .await?;
    writer.write_all(&payload).await?;
    Ok(())
}

pub async fn read_control_frame<R>(reader: &mut R) -> Result<ControlFrame, TunnelStreamError>
where
    R: AsyncRead + Unpin,
{
    let mut prefix = [0_u8; HEADER_LEN_SIZE];
    reader.read_exact(&mut prefix).await?;

    let frame_len = u32::from_be_bytes(prefix) as usize;
    if frame_len > MAX_HEADER_LEN {
        return Err(TunnelStreamError::HeaderTooLarge { size: frame_len });
    }

    let mut payload = vec![0_u8; frame_len];
    reader.read_exact(&mut payload).await?;
    Ok(serde_json::from_slice(&payload)?)
}
