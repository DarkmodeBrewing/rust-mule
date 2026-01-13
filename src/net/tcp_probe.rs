use std::time::Duration;
use tokio::{net::TcpStream, time::timeout};

pub async fn tcp_probe(target: &str, connect_timeout: Duration) -> anyhow::Result<()> {
    let _span = tracing::info_span!("tcp_probe", %target).entered();

    match timeout(connect_timeout, TcpStream::connect(target)).await {
        Ok(Ok(_stream)) => {
            tracing::info!("tcp connect OK");
            Ok(())
        }
        Ok(Err(err)) => {
            tracing::warn!(error = %err, "tcp connect failed");
            Err(err.into())
        }
        Err(_) => {
            tracing::warn!("tcp connect timed out");
            Err(anyhow::anyhow!(
                "tcp connect timed out after {:?}",
                connect_timeout
            ))
        }
    }
}
