use anyhow::Context;

/// Server2 listens for server1 messages and tries to connect to client
#[derive(Debug, clap::Parser)]
#[clap(version, about, long_about = None)]
struct Args {
    /// Server2 UDP listen address
    #[clap(short, long)]
    local_address: std::net::SocketAddr,
}

async fn handle_client(
    encoded_client_address: &[u8],
    server2_sock: std::sync::Arc<tokio::net::UdpSocket>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Decode client's address
    let decoded_client_addr: std::net::SocketAddr =
        bincode::deserialize(&encoded_client_address)
            .context("Failed to decode client address")?;

    // Send client SERVER2_MESSAGE
    use nat_type_rs::protocol;
    if server2_sock
        .send_to(&protocol::SERVER2_MESSAGE, decoded_client_addr)
        .await
        .context("Failed to send SERVER2_MESSAGE")?
        != protocol::SERVER2_MESSAGE.len()
    {
        return Err(anyhow::anyhow!("Did not send entire SERVER2_MESSAGE").into());
    }
    return Ok(());
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    use clap::Parser;
    let args = Args::parse();

    let server2_sock = tokio::net::UdpSocket::bind(args.local_address)
        .await
        .context("Failed to bind server2_sock")?;
    log::debug!("Server2 bound to {}", server2_sock.local_addr().unwrap());
    let server2_sock = std::sync::Arc::new(server2_sock);

    let mut recv_buf = [0_u8; u16::MAX as usize];
    loop {
        match server2_sock.recv_from(&mut recv_buf).await {
            Ok((size, remote_addr)) => {
                log::info!("Incoming connection from {}", remote_addr);
                let cloned_sock = server2_sock.clone();
                tokio::spawn(async move {
                    if let Err(e) =
                        handle_client(&recv_buf[..size], cloned_sock).await
                    {
                        log::error!("Error when processing client: {}", e);
                    }
                });
            }
            Err(e) => log::error!("Error reading from server2_sock: {}", e),
        }
    }
}
