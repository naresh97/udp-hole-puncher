use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
pub struct Cli {
    /// Local port to bind to
    #[clap(short = 'p', long)]
    pub local_tunnel_port: u16,

    /// Remote port to connect to
    #[clap(short = 'q', long)]
    pub remote_tunnel_port: u16,

    /// Remote address to connect to
    #[clap(short = 'r', long)]
    pub remote_addr: String,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    Listen { listen_on_port: u16 },
    Forward { forward_to_port: u16 },
}
