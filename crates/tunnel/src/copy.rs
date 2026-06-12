use std::sync::Arc;

use tokio::io::{self, AsyncRead, AsyncWrite};

use crate::stats::AtomicTrafficStats;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopyOutcome {
    pub uplink_bytes: u64,
    pub downlink_bytes: u64,
}

pub async fn copy_bidirectional_with_stats<A, B>(
    mut uplink: A,
    mut downlink: B,
    stats: Arc<AtomicTrafficStats>,
) -> io::Result<CopyOutcome>
where
    A: AsyncRead + AsyncWrite + Unpin,
    B: AsyncRead + AsyncWrite + Unpin,
{
    stats.begin_stream();
    let result = io::copy_bidirectional(&mut uplink, &mut downlink).await;
    stats.end_stream();

    let (uplink_bytes, downlink_bytes) = result?;
    stats.add_uplink(uplink_bytes);
    stats.add_downlink(downlink_bytes);

    Ok(CopyOutcome {
        uplink_bytes,
        downlink_bytes,
    })
}
