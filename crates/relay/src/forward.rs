use quic_tunnel_tunnel::{
    copy::{copy_bidirectional_with_stats, CopyOutcome},
    stats::AtomicTrafficStats,
    stream::{read_data_header, write_data_header, TunnelStreamError},
};
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::limiter::RelayLimiter;

pub async fn forward_stream_pair<M, A>(
    mut mobile_stream: M,
    mut agent_stream: A,
) -> Result<CopyOutcome, RelayForwardError>
where
    M: AsyncRead + AsyncWrite + Unpin,
    A: AsyncRead + AsyncWrite + Unpin,
{
    let header = read_data_header(&mut mobile_stream).await?;
    write_data_header(&mut agent_stream, &header).await?;

    Ok(copy_bidirectional_with_stats(
        mobile_stream,
        agent_stream,
        Arc::new(AtomicTrafficStats::default()),
    )
    .await?)
}

pub async fn forward_stream_pair_with_limit<M, A>(
    mut mobile_stream: M,
    mut agent_stream: A,
    limiter: RelayLimiter,
) -> Result<CopyOutcome, RelayForwardError>
where
    M: AsyncRead + AsyncWrite + Unpin,
    A: AsyncRead + AsyncWrite + Unpin,
{
    let header = read_data_header(&mut mobile_stream).await?;
    write_data_header(&mut agent_stream, &header).await?;

    Ok(copy_bidirectional_with_limiter(mobile_stream, agent_stream, limiter).await?)
}

async fn copy_bidirectional_with_limiter<A, B>(
    uplink: A,
    downlink: B,
    limiter: RelayLimiter,
) -> std::io::Result<CopyOutcome>
where
    A: AsyncRead + AsyncWrite + Unpin,
    B: AsyncRead + AsyncWrite + Unpin,
{
    let (mut uplink_reader, mut uplink_writer) = tokio::io::split(uplink);
    let (mut downlink_reader, mut downlink_writer) = tokio::io::split(downlink);

    let uplink_limiter = limiter.clone();
    let uplink = copy_one_direction(&mut uplink_reader, &mut downlink_writer, uplink_limiter);
    let downlink = copy_one_direction(&mut downlink_reader, &mut uplink_writer, limiter);
    let (uplink_bytes, downlink_bytes) = tokio::try_join!(uplink, downlink)?;

    Ok(CopyOutcome {
        uplink_bytes,
        downlink_bytes,
    })
}

async fn copy_one_direction<R, W>(
    reader: &mut R,
    writer: &mut W,
    limiter: RelayLimiter,
) -> std::io::Result<u64>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buf = [0_u8; 8 * 1024];
    let mut copied = 0_u64;
    loop {
        let n = match reader.read(&mut buf).await {
            Ok(n) => n,
            Err(error) if is_graceful_stream_close(&error) => {
                let _ = writer.shutdown().await;
                return Ok(copied);
            }
            Err(error) => return Err(error),
        };
        if n == 0 {
            writer.shutdown().await?;
            return Ok(copied);
        }

        limiter.throttle(n).await;
        writer.write_all(&buf[..n]).await?;
        copied = copied.saturating_add(n as u64);
    }
}

fn is_graceful_stream_close(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::NotConnected
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::BrokenPipe
    )
}

#[derive(Debug, thiserror::Error)]
pub enum RelayForwardError {
    #[error(transparent)]
    Stream(#[from] TunnelStreamError),
    #[error("stream copy failed: {0}")]
    Io(#[from] std::io::Error),
}
