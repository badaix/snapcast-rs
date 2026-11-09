//! Snapcast client implementation

use clap::Parser;
use log::info;
use snapcast_proto;
use snapcast_proto::player;
use std::error::Error;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Snapserver host
    #[arg(long)]
    host: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    let args = Args::parse();

    let mut player = player::Player::new();
    player.play();
    info!("Connecting to {}!", args.host);
    snapcast_proto::connect(&args.host).await?;
    Ok(())
}
