use clap::Parser;
use snapcast_proto;
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
    let args = Args::parse();

    println!("Connecting to {}!", args.host);
    snapcast_proto::connect(&args.host).await?;
    Ok(())
}
