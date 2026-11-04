use sigmaos_keyring::{Config, KeyringComponent, Component};
fn main() {
    env_logger::init();
    let mut c = KeyringComponent::new(Config::default());
    if let Err(e) = c.initialize() { eprintln!("Failed: {}", e); std::process::exit(1); }
    println!("SigmaOS keyring running");
    let _ = c.shutdown();
}
