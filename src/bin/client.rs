/// Client program to test router's NAT type
#[derive(Debug, clap::Parser)]
#[clap(version, about, long_about = None)]
struct Args {
    /// CLient UDP bind  address
    #[clap(short, long)]
    local_address: std::net::SocketAddr,

    /// Server1 listen address
    #[clap(short, long)]
    remote_address1: std::net::SocketAddr,
}

fn get_unspec_sock_addr(base: &std::net::SocketAddr) -> std::net::SocketAddr {
    return match base {
        std::net::SocketAddr::V4(_) => std::net::SocketAddr::V4(
            std::net::SocketAddrV4::new(std::net::Ipv4Addr::UNSPECIFIED, 0),
        ),
        std::net::SocketAddr::V6(_) => std::net::SocketAddr::V6(
            std::net::SocketAddrV6::new(std::net::Ipv6Addr::UNSPECIFIED, 0, 0, 0),
        ),
    };
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    use clap::Parser;
    let args = Args::parse();

    use anyhow::Context;
    let client_sock = tokio::net::UdpSocket::bind(args.local_address)
        .await
        .context("Failed to bind local socket")?;
    log::debug!("Client bound to {}", client_sock.local_addr().unwrap());

    client_sock
        .connect(args.remote_address1)
        .await
        .context("Failed to connect to remote socket")?;
    log::debug!(
        "Client conntected to {}",
        client_sock.peer_addr().context("Socket not connected")?
    );

    // Send hello
    use nat_type_rs::protocol;
    if client_sock
        .send(&protocol::HELLO_MESSAGE)
        .await
        .context("Failed to send hello message")?
        != protocol::HELLO_MESSAGE.len()
    {
        return Err(anyhow::anyhow!("Did not send entire message").into());
    }
    // Recieve our address from server1 side
    let mut buf = [0_u8; u16::MAX as usize];
    let buf_len = client_sock.recv(&mut buf).await.context("Failed to recv")?;
    let decoded_client_address: std::net::SocketAddr =
        bincode::deserialize(&buf[..buf_len])
            .context("Failed to deserialize client address")?;
    log::info!(
        "Server1 returned our address from its perspective: {}",
        decoded_client_address
    );

    // Try to recieve NEW_SERVER1_MESSAGE from different port
    // Unconnect client_sock
    client_sock
        .connect(get_unspec_sock_addr(&args.remote_address1))
        .await
        .context("Failed to unconnect")?;
    match tokio::time::timeout(
        std::time::Duration::from_secs(2),
        client_sock.recv_from(&mut buf),
    )
    .await
    {
        Ok(result) => {
            let (size, peer_addr) =
                result.context("Failed to recieve NEW_SERVER1_MESSAGE")?;
            log::info!("Client recieved a message from {}", peer_addr);
            if &buf[..size] == protocol::NEW_SERVER1_MESSAGE {
                log::info!(
                    "Client successfully recieved NEW_SERVER1_MESSAGE from {}",
                    peer_addr
                );
                log::info!("Your router probably has Address-restricted-cone NAT");
            } else {
                return Err(anyhow::anyhow!(
                    "Client recieved invalid NEW_SERVER1_MESSAGE"
                )
                .into());
            }
        }
        Err(_) => {
            log::error!("Timeout reached waiting for NEW_SERVER1_MESSAGE");
            log::info!("Your router probably has Port-restricted-cone NAT");
        }
    }

    // Try to recieve SERVER2_MESSAGE from different host
    match tokio::time::timeout(
        std::time::Duration::from_secs(2),
        client_sock.recv_from(&mut buf),
    )
    .await
    {
        Ok(result) => {
            let (size, peer_addr) =
                result.context("Failed to recieve SERVER2_MESSAGE")?;
            log::info!("Client recieved a message from {}", peer_addr);
            if &buf[..size] == protocol::SERVER2_MESSAGE {
                log::info!(
                    "Client successfully recieved SERVER2_MESSAGE from {}",
                    peer_addr
                );
                log::info!("Your router probably has Full-cone NAT");
            } else {
                return Err(anyhow::anyhow!(
                    "Client recieved invalid SERVER2_MESSAGE"
                )
                .into());
            }
        }
        Err(_) => {
            log::error!("Timeout reached waiting for SERVER2_MESSAGE");
            log::info!("Either Server2 is not running or your router probably has Address-restricted-cone NAT");
        }
    }

    return Ok(());
}
