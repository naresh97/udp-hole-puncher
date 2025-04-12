use tokio::{
    net::{TcpListener, TcpStream, UdpSocket},
    sync::mpsc::{Receiver, Sender},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    let local_port = args
        .get(1)
        .expect("port not specified")
        .parse::<u16>()
        .expect("port not a number");
    let remote_addr = args.get(2).expect("addr not specified");
    let remote_port = args
        .get(3)
        .expect("port not specified")
        .parse::<u16>()
        .expect("port not a number");
    let peer_port = args
        .get(4)
        .expect("port not specified")
        .parse::<u16>()
        .expect("port not a number");

    let (tx_to_peer, mut rx_from_peer) = tokio::sync::mpsc::channel::<Vec<u8>>(32);
    let (tx_to_local, mut rx_from_local) = tokio::sync::mpsc::channel::<Vec<u8>>(32);

    local_listener(peer_port, tx_to_peer, rx_from_local).await?;
    puncher(
        local_port,
        &remote_addr,
        remote_port,
        tx_to_local,
        rx_from_peer,
    )
    .await?;
    Ok(())
}

async fn puncher(
    local_port: u16,
    remote_addr: &str,
    remote_port: u16,
    tx: Sender<Vec<u8>>,
    mut rx: Receiver<Vec<u8>>,
) -> anyhow::Result<()> {
    let socket = UdpSocket::bind(("0.0.0.0", local_port)).await?;
    socket.connect((remote_addr, remote_port)).await?;
    let mut received_ack = false;
    let mut received_punch = false;
    loop {
        if received_ack && received_punch {
            break;
        }
        tokio::select! {
            _ =  socket.readable() => {
                let mut buf = [0; 1024];
                let _ = socket.try_recv(&mut buf);
                let msg = String::from_utf8_lossy(&buf);
                if msg.contains("PUNCH") && !msg.contains("ACK_PUNCH") {
                    let _ = socket.try_send("ACK_PUNCH".as_bytes());
                    received_punch = true;
                }else if msg.contains("ACK_PUNCH") {
                    received_ack = true;
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {
                println!("Sending PUNCH");
                let _ = socket.send("PUNCH".as_bytes()).await;
            }
        };
    }
    println!("Punched through to {}:{}", remote_addr, remote_port);
    loop {
        socket.readable().await?;
        let mut buf = [0; 1024];
        match socket.try_recv(&mut buf) {
            Ok(n) => {
                if n == 0 {
                    println!("Peer disconnected");
                    break;
                }
                tx.send(buf[..n].to_vec()).await?;
                let response = rx
                    .recv()
                    .await
                    .ok_or(anyhow::anyhow!("Failed to receive response"))?;
                loop {
                    match socket.try_send(&response) {
                        Ok(_) => break,
                        Err(e) => {
                            if e.kind() == std::io::ErrorKind::WouldBlock {
                                continue;
                            } else {
                                break;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::WouldBlock {
                    continue;
                } else {
                    break;
                }
            }
        }
    }

    Ok(())
}

async fn local_listener(
    port: u16,
    tx: Sender<Vec<u8>>,
    rx: Receiver<Vec<u8>>,
) -> anyhow::Result<()> {
    println!("Listening on port {}", port);
    let listener = TcpListener::bind(("0.0.0.0", port)).await?;
    let (socket, _) = listener.accept().await?;
    handle_client(socket, tx, rx).await
}

async fn handle_client(
    socket: TcpStream,
    tx: Sender<Vec<u8>>,
    mut rx: Receiver<Vec<u8>>,
) -> anyhow::Result<()> {
    println!("Accepted connection from {}", socket.peer_addr().unwrap());
    loop {
        socket.readable().await?;
        let mut buf = [0; 1024];
        match socket.try_read(&mut buf) {
            Ok(n) => {
                if n == 0 {
                    println!("Client disconnected");
                    break;
                }
                tx.send(buf[..n].to_vec()).await?;
                let response = rx
                    .recv()
                    .await
                    .ok_or(anyhow::anyhow!("Failed to receive response"))?;
                loop {
                    match socket.try_write(&response) {
                        Ok(_) => break,
                        Err(e) => {
                            if e.kind() == std::io::ErrorKind::WouldBlock {
                                continue;
                            } else {
                                break;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::WouldBlock {
                    continue;
                } else {
                    break;
                }
            }
        }
    }
    Ok(())
}
