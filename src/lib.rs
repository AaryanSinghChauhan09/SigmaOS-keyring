// SPDX-License-Identifier: Apache-2.0
//! SigmaOS-keyring — freedesktop Secret Service keyring.
//!
//! Models keyrings (collections) and secret items with attributes,
//! lock/unlock semantics, and search — the same contract gnome-keyring /
//! KWallet expose over D-Bus. Secrets are stored as opaque bytes; a real
//! backend (kernel keyring / libsecret) can replace the in-memory store.

use std::collections::BTreeMap;
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum KeyringKind { Login, Session, Default, Ephemeral }
impl KeyringKind {
    pub fn label(self) -> &'static str {
        match self { Self::Login=>"login", Self::Session=>"session", Self::Default=>"default", Self::Ephemeral=>"ephemeral" }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Keyring {
    pub id: String,
    pub name: String,             // "login", "session", custom
    pub kind: KeyringKind,
    pub locked: bool,
    pub created: u64,
    pub modified: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SecretItem {
    pub id: String,
    pub label: String,
    pub attributes: BTreeMap<String, String>, // e.g. {"service":"github","username":"aaryan"}
    pub secret: Vec<u8>,          // opaque secret bytes (never logged)
    pub content_type: String,     // "text/plain", "application/octet-stream"
    pub created: u64,
    pub modified: u64,
}

impl SecretItem {
    pub fn get_attr(&self, k: &str) -> Option<&str> { self.attributes.get(k).map(|s| s.as_str()) }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyringError {
    EmptyId,
    DuplicateKeyring(String),
    DuplicateItem { keyring: String, item: String },
    NoSuchKeyring(String),
    NoSuchItem { keyring: String, item: String },
    Locked(String),
    EmptyLabel,
    BadIndex,
}
impl fmt::Display for KeyringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyId => write!(f, "id must not be empty"),
            Self::DuplicateKeyring(k) => write!(f, "duplicate keyring {k}"),
            Self::DuplicateItem { keyring, item } => write!(f, "duplicate item {item} in {keyring}"),
            Self::NoSuchKeyring(k) => write!(f, "no such keyring {k}"),
            Self::NoSuchItem { keyring, item } => write!(f, "no such item {item} in {keyring}"),
            Self::Locked(k) => write!(f, "keyring {k} is locked"),
            Self::EmptyLabel => write!(f, "item label must not be empty"),
            Self::BadIndex => write!(f, "bad memory index"),
        }
    }
}
impl std::error::Error for KeyringError {}

pub struct SigmaOSKeyring {
    keyrings: BTreeMap<String, Keyring>,
    items: BTreeMap<String, Vec<SecretItem>>,   // keyring_id -> items
    next_item: u64,
}

impl SigmaOSKeyring {
    pub fn new() -> Self {
        let mut m = Self { keyrings: BTreeMap::new(), items: BTreeMap::new(), next_item: 0 };
        // standard collections
        for (id, kind) in [("login", KeyringKind::Login), ("session", KeyringKind::Session)] {
            let t = now();
            m.keyrings.insert(id.into(), Keyring {
                id: id.into(), name: id.into(), kind,
                locked: kind == KeyringKind::Login, // login starts locked
                created: t, modified: t,
            });
            m.items.insert(id.into(), Vec::new());
        }
        m
    }

    // ---- keyrings ----
    pub fn create_keyring(&mut self, id: impl Into<String>, name: impl Into<String>, kind: KeyringKind)
        -> Result<(), KeyringError>
    {
        let id = id.into();
        if id.trim().is_empty() { return Err(KeyringError::EmptyId); }
        if self.keyrings.contains_key(&id) { return Err(KeyringError::DuplicateKeyring(id)); }
        let t = now();
        self.keyrings.insert(id.clone(), Keyring {
            id: id.clone(), name: name.into(), kind, locked: false, created: t, modified: t,
        });
        self.items.insert(id, Vec::new());
        Ok(())
    }

    pub fn keyring(&self, id: &str) -> Option<&Keyring> { self.keyrings.get(id) }
    pub fn keyrings(&self) -> impl Iterator<Item = &Keyring> { self.keyrings.values() }

    pub fn unlock(&mut self, id: &str) -> Result<(), KeyringError> {
        let k = self.keyrings.get_mut(id).ok_or_else(|| KeyringError::NoSuchKeyring(id.into()))?;
        k.locked = false; k.modified = now(); Ok(())
    }
    pub fn lock(&mut self, id: &str) -> Result<(), KeyringError> {
        let k = self.keyrings.get_mut(id).ok_or_else(|| KeyringError::NoSuchKeyring(id.into()))?;
        k.locked = true; k.modified = now(); Ok(())
    }
    pub fn is_locked(&self, id: &str) -> Result<bool, KeyringError> {
        Ok(self.keyrings.get(id).ok_or_else(|| KeyringError::NoSuchKeyring(id.into()))?.locked)
    }

    // ---- items ----
    fn items_mut(&mut self, keyring: &str) -> Result<&mut Vec<SecretItem>, KeyringError> {
        self.items.get_mut(keyring).ok_or_else(|| KeyringError::NoSuchKeyring(keyring.into()))
    }
    fn check_unlocked(&self, keyring: &str) -> Result<(), KeyringError> {
        let k = self.keyrings.get(keyring).ok_or_else(|| KeyringError::NoSuchKeyring(keyring.into()))?;
        if k.locked { Err(KeyringError::Locked(keyring.into())) } else { Ok(()) }
    }

    /// Store a secret in a keyring.
    pub fn store(&mut self, keyring: &str, label: impl Into<String>,
                 attributes: BTreeMap<String, String>, secret: Vec<u8>, content_type: impl Into<String>)
        -> Result<String, KeyringError>
    {
        self.check_unlocked(keyring)?;
        let label = label.into();
        if label.trim().is_empty() { return Err(KeyringError::EmptyLabel); }
        let dup = self.items.get(keyring).map(|v| v.iter().any(|i| i.attributes == attributes)).unwrap_or(false);
        if dup {
            return Err(KeyringError::DuplicateItem { keyring: keyring.into(), item: label });
        }
        self.next_item += 1;
        let id = format!("item-{}", self.next_item);
        let t = now();
        self.items_mut(keyring)?.push(SecretItem {
            id: id.clone(), label, attributes, secret,
            content_type: content_type.into(), created: t, modified: t,
        });
        if let Some(k) = self.keyrings.get_mut(keyring) { k.modified = t; }
        Ok(id)
    }

    pub fn get(&self, keyring: &str, item_id: &str) -> Result<&SecretItem, KeyringError> {
        self.check_unlocked(keyring)?;
        self.items.get(keyring).and_then(|v| v.iter().find(|i| i.id == item_id))
            .ok_or_else(|| KeyringError::NoSuchItem { keyring: keyring.into(), item: item_id.into() })
    }

    /// Search items by attribute subset (all given attrs must match).
    pub fn search(&self, keyring: &str, attrs: &BTreeMap<String, String>) -> Result<Vec<&SecretItem>, KeyringError> {
        self.check_unlocked(keyring)?;
        let items = self.items.get(keyring).ok_or_else(|| KeyringError::NoSuchKeyring(keyring.into()))?;
        Ok(items.iter().filter(|i| attrs.iter().all(|(k, v)| i.attributes.get(k) == Some(v))).collect())
    }

    /// Convenience: find secret bytes by attribute lookup.
    pub fn lookup(&self, keyring: &str, service: &str, username: &str) -> Result<&[u8], KeyringError> {
        let mut attrs = BTreeMap::new();
        attrs.insert("service".to_string(), service.to_string());
        attrs.insert("username".to_string(), username.to_string());
        let hits = self.search(keyring, &attrs)?;
        hits.first().map(|i| i.secret.as_slice())
            .ok_or_else(|| KeyringError::NoSuchItem { keyring: keyring.into(), item: format!("{service}/{username}") })
    }

    pub fn delete(&mut self, keyring: &str, item_id: &str) -> Result<SecretItem, KeyringError> {
        self.check_unlocked(keyring)?;
        let items = self.items_mut(keyring)?;
        let pos = items.iter().position(|i| i.id == item_id)
            .ok_or_else(|| KeyringError::NoSuchItem { keyring: keyring.into(), item: item_id.into() })?;
        let removed = items.remove(pos);
        if let Some(k) = self.keyrings.get_mut(keyring) { k.modified = now(); }
        Ok(removed)
    }

    pub fn items(&self, keyring: &str) -> Result<impl Iterator<Item = &SecretItem>, KeyringError> {
        self.check_unlocked(keyring)?;
        Ok(self.items.get(keyring).ok_or_else(|| KeyringError::NoSuchKeyring(keyring.into()))?.iter())
    }

    /// List item metadata (labels + attrs, NOT secrets) for a keyring.
    pub fn list(&self, keyring: &str) -> Result<Vec<(String, String, BTreeMap<String, String>)>, KeyringError> {
        self.check_unlocked(keyring)?;
        let items = self.items.get(keyring).ok_or_else(|| KeyringError::NoSuchKeyring(keyring.into()))?;
        Ok(items.iter().map(|i| (i.id.clone(), i.label.clone(), i.attributes.clone())).collect())
    }

    /// Persist to a JSON file (dev backend; a real Secret Service uses
    /// an encrypted store — this keeps CLI state across invocations).
    pub fn save_to(&self, path: &std::path::Path) -> Result<(), String> {
        #[derive(serde::Serialize, serde::Deserialize)]
        struct Disk {
            keyrings: Vec<Keyring>,
            items: BTreeMap<String, Vec<SecretItem>>,
            next_item: u64,
        }
        let d = Disk { keyrings: self.keyrings.values().cloned().collect(), items: self.items.clone(), next_item: self.next_item };
        let j = serde_json::to_string_pretty(&d).map_err(|e| e.to_string())?;
        if let Some(p) = path.parent() { std::fs::create_dir_all(p).map_err(|e| e.to_string())?; }
        std::fs::write(path, j).map_err(|e| e.to_string())
    }

    pub fn load_from(path: &std::path::Path) -> Result<Self, String> {
        #[derive(serde::Serialize, serde::Deserialize)]
        struct Disk {
            keyrings: Vec<Keyring>,
            items: BTreeMap<String, Vec<SecretItem>>,
            next_item: u64,
        }
        let txt = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let d: Disk = serde_json::from_str(&txt).map_err(|e| e.to_string())?;
        let mut keyrings = BTreeMap::new();
        for k in d.keyrings { keyrings.insert(k.id.clone(), k); }
        Ok(Self { keyrings, items: d.items, next_item: d.next_item })
    }

    /// Default state path: $SIGMAOS_KEYRING_FILE or ~/.local/share/sigmaos/keyring.json
    pub fn state_path() -> std::path::PathBuf {
        if let Ok(p) = std::env::var("SIGMAOS_KEYRING_FILE") { return std::path::PathBuf::from(p); }
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        std::path::PathBuf::from(home).join(".local/share/sigmaos/keyring.json")
    }

    pub fn status_line(&self) -> String {
        let total_items: usize = self.items.values().map(|v| v.len()).sum();
        let locked: usize = self.keyrings.values().filter(|k| k.locked).count();
        format!("keyring: {} keyrings ({} locked), {} items", self.keyrings.len(), locked, total_items)
    }
}

impl Default for SigmaOSKeyring {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attrs(svc: &str, user: &str) -> BTreeMap<String, String> {
        let mut m = BTreeMap::new();
        m.insert("service".into(), svc.into());
        m.insert("username".into(), user.into());
        m
    }

    #[test]
    fn login_locked_by_default() {
        let m = SigmaOSKeyring::new();
        assert!(m.is_locked("login").unwrap());
        assert!(!m.is_locked("session").unwrap());
        // can't store into locked login
        let mut m2 = SigmaOSKeyring::new();
        assert!(m2.store("login", "pw", attrs("x","y"), b"s".to_vec(), "text/plain").is_err());
    }

    #[test]
    fn store_get_lookup() {
        let mut m = SigmaOSKeyring::new();
        m.unlock("login").unwrap();
        let id = m.store("login", "GitHub PAT", attrs("github","aaryan"), b"ghp_secret".to_vec(), "text/plain").unwrap();
        assert!(id.starts_with("item-"));
        let item = m.get("login", &id).unwrap();
        assert_eq!(item.secret, b"ghp_secret");
        assert_eq!(m.lookup("login", "github", "aaryan").unwrap(), b"ghp_secret");
    }

    #[test]
    fn search_by_attrs() {
        let mut m = SigmaOSKeyring::new();
        m.store("session", "wifi-home", attrs("wifi","home"), b"psk123".to_vec(), "text/plain").unwrap();
        m.store("session", "wifi-cafe", attrs("wifi","cafe"), b"cafe456".to_vec(), "text/plain").unwrap();
        let hits = m.search("session", &attrs("wifi","home")).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].label, "wifi-home");
        // partial attr match (only service)
        let mut only_svc = BTreeMap::new(); only_svc.insert("service".into(), "wifi".into());
        assert_eq!(m.search("session", &only_svc).unwrap().len(), 2);
    }

    #[test]
    fn dedup_and_delete() {
        let mut m = SigmaOSKeyring::new();
        m.store("session", "a", attrs("s","u"), b"1".to_vec(), "text/plain").unwrap();
        assert!(m.store("session", "b", attrs("s","u"), b"2".to_vec(), "text/plain").is_err()); // same attrs
        let id = m.store("session", "c", attrs("s","v"), b"3".to_vec(), "text/plain").unwrap();
        let removed = m.delete("session", &id).unwrap();
        assert_eq!(removed.label, "c");
        assert!(m.get("session", &id).is_err());
    }

    #[test]
    fn custom_keyring_and_lock_cycle() {
        let mut m = SigmaOSKeyring::new();
        m.create_keyring("work", "Work Keyring", KeyringKind::Default).unwrap();
        m.store("work", "corp", attrs("vpn","corp"), b"tok".to_vec(), "text/plain").unwrap();
        m.lock("work").unwrap();
        assert!(m.get("work", "item-1").is_err()); // locked
        m.unlock("work").unwrap();
        assert!(m.get("work", "item-1").is_ok());
        assert!(m.create_keyring(" ", "x", KeyringKind::Default).is_err());
        assert!(m.create_keyring("work", "dup", KeyringKind::Default).is_err());
    }

    #[test]
    fn list_hides_secrets() {
        let mut m = SigmaOSKeyring::new();
        m.store("session", "secret-item", attrs("a","b"), b"TOPSECRET".to_vec(), "text/plain").unwrap();
        let list = m.list("session").unwrap();
        assert_eq!(list.len(), 1);
        // ensure secret bytes not exposed in listing (only id/label/attrs)
        let (_, label, a) = &list[0];
        assert_eq!(label, "secret-item");
        // attributes are metadata (fine to list); only secret BYTES are hidden
        assert_eq!(a.get("service"), Some(&"a".to_string()));
        assert!(!format!("{:?}", list).contains("TOPSECRET"));
    }

    #[test]
    fn errors() {
        let mut m = SigmaOSKeyring::new();
        assert!(m.store("nope", "x", BTreeMap::new(), vec![], "text/plain").is_err());
        assert!(m.get("session", "nope").is_err());
        assert!(m.delete("session", "nope").is_err());
        assert!(m.store("session", "  ", BTreeMap::new(), vec![], "text/plain").is_err());
        assert!(m.status_line().contains("keyrings"));
    }
}
