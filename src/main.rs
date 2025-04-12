use tokio::net::{TcpListener, TcpStream, UdpSocket};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    let local_port = args
        .get(1)
        .expect("port not specified")
        .parse::<u16>()
        .expect("port not a number");
    let remote_port = args
        .get(2)
        .expect("port not specified")
        .parse::<u16>()
        .expect("port not a number");
    //local_listener(local_port).await?;
    puncher(local_port, "127.0.0.1", remote_port).await?;
    Ok(())
}

async fn puncher(local_port: u16, remote_addr: &str, remote_port: u16) -> anyhow::Result<()> {
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

    Ok(())
}

async fn local_listener(port: u16) -> anyhow::Result<()> {
    println!("Listening on port {}", port);
    let listener = TcpListener::bind(("0.0.0.0", port)).await?;
    loop {
        let (socket, _) = listener.accept().await?;
        tokio::spawn(handle_client(socket));
    }
}

async fn handle_client(socket: TcpStream) -> anyhow::Result<()> {
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
                let response = forward_to_peer(&buf[..n]).await?;
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

async fn forward_to_peer(data: &[u8]) -> anyhow::Result<Vec<u8>> {
    println!("Forwarding data: {:?}", data);
    let response = format!("Received: {:?}\n", data);
    let response = response.as_bytes().to_vec();
    Ok(response)
}
