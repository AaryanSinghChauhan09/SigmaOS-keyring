use anyhow::Result;
use clap::Parser;
use keyring::keyring;

#[derive(Parser)]
struct Cli {
    #[arg(short, long)]
    auto: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = keyring::Config { auto_start: cli.auto, ..Default::default() };
    let mut component = keyring::new(config);
    
    component.initialize()?;
    component.run()?;
    
    println!("SigmaOS-keyring operational");
    Ok(())
}
