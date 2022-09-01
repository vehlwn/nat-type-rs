use anyhow::Context;

/// Server1 listens for client requests and optionally conntects to the server2
#[derive(Debug, clap::Parser)]
#[clap(version, about, long_about = None)]
struct Args {
    /// Server1 UDP bind address
    #[clap(short, long)]
    local_address: std::net::SocketAddr,

    /// Server2 listen address
    #[clap(short, long)]
    remote_address2: Option<std::net::SocketAddr>,
}

fn gen_not_equal_port(target: u16) -> u16 {
    let mut gen = rand::thread_rng();
    let dist = rand::distributions::Uniform::from(1024..=u16::MAX);
    loop {
        use rand::distributions::Distribution;
        let result = dist.sample(&mut gen);
        if result != target {
            return result;
        }
    }
}

async fn handle_client(
    client_address: std::net::SocketAddr,
    hello_message: &[u8],
    server1_sock: std::sync::Arc<tokio::net::UdpSocket>,
    remote_address2: Option<std::net::SocketAddr>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check hello message
    use nat_type_rs::protocol;
    if hello_message != protocol::HELLO_MESSAGE {
        return Err(anyhow::anyhow!("Bad hello message").into());
    }

    // Send apparent address back to the client
    let client_encoded_addr: Vec<u8> = bincode::serialize(&client_address).unwrap();
    if server1_sock
        .send_to(&client_encoded_addr, client_address)
        .await
        .context("Failed to send client_address")?
        != client_encoded_addr.len()
    {
        return Err(
            anyhow::anyhow!("Did not send entire client_address message").into(),
        );
    }

    // Try to send a message from a different port
    let new_bind_addr = {
        let mut tmp = server1_sock.local_addr().unwrap();
        tmp.set_port(gen_not_equal_port(tmp.port()));
        tmp
    };
    let new_server1_sock = tokio::net::UdpSocket::bind(new_bind_addr)
        .await
        .context("Failed to bind new_server1_sock socket")?;
    log::debug!(
        "Server1 with new port bound to {}",
        new_server1_sock.local_addr().unwrap()
    );
    if new_server1_sock
        .send_to(&protocol::NEW_SERVER1_MESSAGE, client_address)
        .await
        .context("Failed to send NEW_SERVER1_MESSAGE")?
        != protocol::NEW_SERVER1_MESSAGE.len()
    {
        return Err(
            anyhow::anyhow!("Did not send entire NEW_SERVER1_MESSAGE").into()
        );
    }
    log::debug!(
        "Server1 sent NEW_SERVER1_MESSAGE to {} from {}",
        client_address,
        new_server1_sock.local_addr().unwrap()
    );

    // Send client_address to server2, if present
    if let Some(remote_address2) = remote_address2 {
        if server1_sock
            .send_to(&client_encoded_addr, remote_address2)
            .await
            .context("Failed to send client_address to server2")?
            != client_encoded_addr.len()
        {
            return Err(anyhow::anyhow!(
                "Did not send entire client_address to server2"
            )
            .into());
        }
        log::debug!("Server1 sent client_address to server2")
    }
    return Ok(());
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    use clap::Parser;
    let args = Args::parse();

    let server1_sock = tokio::net::UdpSocket::bind(args.local_address)
        .await
        .context("Failed to bind server1_sock")?;
    log::debug!("Server1 bound to {}", server1_sock.local_addr().unwrap());
    let server1_sock = std::sync::Arc::new(server1_sock);

    let mut recv_buf = [0_u8; u16::MAX as usize];
    loop {
        match server1_sock.recv_from(&mut recv_buf).await {
            Ok((size, remote_addr)) => {
                log::info!("Incoming connection from {}", remote_addr);
                let cloned_sock = server1_sock.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_client(
                        remote_addr,
                        &recv_buf[..size],
                        cloned_sock,
                        args.remote_address2,
                    )
                    .await
                    {
                        log::error!("Error when processing client: {}", e);
                    }
                });
            }
            Err(e) => log::error!("Error reading from server1_sock: {}", e),
        }
    }
}
