use std::net::SocketAddr;

use anyhow::{Context, Result};
use clap::Parser;
use quic_tunnel_punch::server::PunchServer;

#[derive(Debug, Parser)]
#[command(name = "punch-server")]
#[command(about = "Standalone UDP Punch service for MobileCode Connect candidate discovery")]
struct Cli {
    #[arg(long, default_value = "127.0.0.1:3478")]
    bind: SocketAddr,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let cli = Cli::parse();
    let server = PunchServer::bind(cli.bind)
        .await
        .with_context(|| format!("bind punch server on {}", cli.bind))?;
    let addr = server.local_addr().context("punch server local addr")?;
    println!("punch-server listening on {addr}");
    server
        .run_until(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_punch_server_args() {
        let cli = Cli::try_parse_from(["punch-server", "--bind", "127.0.0.1:3478"]).unwrap();

        assert_eq!(cli.bind.to_string(), "127.0.0.1:3478");
    }
}
