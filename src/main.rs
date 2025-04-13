mod cli;

use anyhow::Context;
use clap::Parser;
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream, UdpSocket},
    sync::mpsc::{Receiver, Sender},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    println!("cli: {:?}", cli);

    let (tx_to_peer, rx_from_peer) = tokio::sync::mpsc::channel::<Vec<u8>>(32);
    let (tx_to_local, rx_from_local) = tokio::sync::mpsc::channel::<Vec<u8>>(32);

    tokio::spawn(async move {
        match cli.command {
            cli::Command::Listen { listen_on_port } => {
                local_listener(listen_on_port, tx_to_peer, rx_from_local).await?;
            }
            cli::Command::Forward { forward_to_port } => {
                forwarder(forward_to_port, tx_to_peer, rx_from_local).await?;
            }
        }
        Ok::<(), anyhow::Error>(())
    });

    puncher(
        cli.local_tunnel_port,
        &cli.remote_addr,
        cli.remote_tunnel_port,
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
    mut rx: Receiver<Vec<u8>>,
) -> anyhow::Result<()> {
    println!("Listening on port {}", port);
    let listener = TcpListener::bind(("0.0.0.0", port)).await?;
    'connection_loop: loop {
        let (mut socket, _) = listener.accept().await?;
        println!("Accepted connection from {}", socket.peer_addr()?);
        // Accept only one connection at a time
        let mut buf = [0; 1024];
        loop {
            socket.readable().await?;
            match socket.try_read(&mut buf) {
                Ok(0) => {
                    println!("Connection closed");
                    break 'connection_loop;
                }
                Ok(n) => {
                    tx.send(buf[..n].to_vec()).await?;
                    let response = rx
                        .recv()
                        .await
                        .ok_or(anyhow::anyhow!("The channel has been closed"))?;
                    let _ = socket.write(&response).await;
                }
                Err(e) => {
                    println!("Error reading from socket: {}", e);
                    break 'connection_loop;
                }
            }
        }
    }
    Ok(())
}

async fn forwarder(
    port: u16,
    tx: Sender<Vec<u8>>,
    mut rx: Receiver<Vec<u8>>,
) -> anyhow::Result<()> {
    let mut socket = TcpStream::connect(("127.0.0.1", port))
        .await
        .context("Could not connect to 127.0.0.1 on specified port")?;
    loop {
        let message = rx
            .recv()
            .await
            .ok_or(anyhow::anyhow!("The channel has been closed"))?;
        let _ = socket.write(&message).await;
        let mut buf = [0; 1024];
        socket.readable().await?;
        match socket.try_read(&mut buf) {
            Ok(0) => {
                println!("Connection closed");
                break;
            }
            Ok(n) => {
                tx.send(buf[..n].to_vec()).await?;
            }
            Err(e) => {
                println!("Error reading from socket: {}", e);
                break;
            }
        }
    }
    todo!()
}
