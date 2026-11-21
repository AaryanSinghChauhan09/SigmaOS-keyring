//! SigmaOS-keyring — Secret Service keyring CLI.
//!
//! Usage:
//!   sigmaos-keyring status|keyrings|list <kr>
//!   sigmaos-keyring unlock|lock <kr>
//!   sigmaos-keyring store <kr> <label> <secret> [--attr k=v ...]
//!   sigmaos-keyring lookup <kr> <service> <username>
//!   sigmaos-keyring get <kr> <item-id>
//!   sigmaos-keyring delete <kr> <item-id>
//!   sigmaos-keyring new <id> <name> [login|session|default|ephemeral]

use sigmaos_keyring::{KeyringKind, SigmaOSKeyring};
use std::collections::BTreeMap;
use std::process::exit;

fn attrs_of(args: &[String]) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    for a in args {
        if let Some((k, v)) = a.split_once('=') { m.insert(k.to_string(), v.to_string()); }
    }
    m
}

fn kind_of(s: Option<&String>) -> KeyringKind {
    match s.map(|x| x.as_str()) {
        Some("login") => KeyringKind::Login,
        Some("session") => KeyringKind::Session,
        Some("ephemeral") => KeyringKind::Ephemeral,
        _ => KeyringKind::Default,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first().map(|s| s.as_str()) else {
        eprintln!("usage: sigmaos-keyring <status|keyrings|list|unlock|lock|store|lookup|get|delete|new>");
        exit(2);
    };
    let rest = &args[1..];
    let path = SigmaOSKeyring::state_path();
    let mut m = SigmaOSKeyring::load_from(&path).unwrap_or_else(|_| SigmaOSKeyring::new());

    let r: Result<(), String> = (|| {
        match cmd {
            "status" => { println!("{}", m.status_line()); for k in m.keyrings() { println!("  {} [{}] {}", k.id, k.kind.label(), if k.locked {"LOCKED"} else {"open"}); } Ok(()) }
            "keyrings" => { for k in m.keyrings() { println!("{}\t{}\t{}", k.id, k.kind.label(), if k.locked {"locked"} else {"open"}); } Ok(()) }
            "list" => {
                let kr = rest.first().ok_or("list needs keyring")?;
                match m.list(kr) {
                    Ok(items) => { for (id, label, a) in items { println!("{id}\t{label}\t{a:?}"); } Ok(()) }
                    Err(e) => Err(e.to_string()),
                }
            }
            "unlock" => { m.unlock(rest.first().ok_or("unlock needs keyring")?).map_err(|e| e.to_string())?; println!("unlocked"); Ok(()) }
            "lock" => { m.lock(rest.first().ok_or("lock needs keyring")?).map_err(|e| e.to_string())?; println!("locked"); Ok(()) }
            "store" => {
                let kr = rest.first().ok_or("store needs keyring")?;
                let label = rest.get(1).ok_or("store needs label")?;
                let secret = rest.get(2).ok_or("store needs secret")?.as_bytes().to_vec();
                let attr_start = rest.iter().position(|a| a == "--attr").map(|i| i + 1).unwrap_or(rest.len());
                let attrs = attrs_of(&rest[attr_start..]);
                // require service+username attrs for lookup-ability (default to label)
                let mut attrs = attrs;
                attrs.entry("service".into()).or_insert_with(|| label.clone());
                let id = m.store(kr, label, attrs, secret, "text/plain").map_err(|e| e.to_string())?;
                println!("stored {id}"); Ok(())
            }
            "lookup" => {
                let kr = rest.first().ok_or("lookup needs keyring")?;
                let svc = rest.get(1).ok_or("lookup needs service")?;
                let user = rest.get(2).ok_or("lookup needs username")?;
                match m.lookup(kr, svc, user) {
                    Ok(s) => { println!("{}", String::from_utf8_lossy(s)); Ok(()) }
                    Err(e) => Err(e.to_string()),
                }
            }
            "get" => {
                let kr = rest.first().ok_or("get needs keyring")?;
                let id = rest.get(1).ok_or("get needs item-id")?;
                match m.get(kr, id) {
                    Ok(i) => { println!("{}\t{}\t{}", i.id, i.label, String::from_utf8_lossy(&i.secret)); Ok(()) }
                    Err(e) => Err(e.to_string()),
                }
            }
            "delete" => {
                let kr = rest.first().ok_or("delete needs keyring")?;
                let id = rest.get(1).ok_or("delete needs item-id")?;
                m.delete(kr, id).map_err(|e| e.to_string())?; println!("deleted"); Ok(())
            }
            "new" => {
                let id = rest.first().ok_or("new needs id")?;
                let name = rest.get(1).map(|s| s.as_str()).unwrap_or(id);
                let kind = kind_of(rest.get(2));
                m.create_keyring(id, name, kind).map_err(|e| e.to_string())?; println!("created {id}"); Ok(())
            }
            _ => { eprintln!("unknown command {cmd}"); exit(2); }
        }
    })();

    match r {
        Ok(()) => { let _ = m.save_to(&path); }
        Err(e) => { eprintln!("error: {e}"); exit(1); }
    }
}
