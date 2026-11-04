use std::collections::HashMap;
#[derive(Debug, Clone, PartialEq)]
pub enum TrustLevel { Untrusted, Marginal, Full, Ultimate }
#[derive(Debug, Clone)]
pub struct TrustKey { pub fingerprint: String, pub owner: String, pub level: TrustLevel }
impl TrustKey {
    pub fn new(fingerprint: &str, owner: &str, level: TrustLevel) -> Self {
        Self { fingerprint: fingerprint.to_string(), owner: owner.to_string(), level }
    }
}
pub struct TrustStore { keys: HashMap<String, TrustKey> }
impl TrustStore {
    pub fn new() -> Self { Self { keys: HashMap::new() } }
    pub fn add_key(&mut self, key: TrustKey) { self.keys.insert(key.fingerprint.clone(), key); }
    pub fn get_trust(&self, fingerprint: &str) -> TrustLevel {
        self.keys.get(fingerprint).map(|k| k.level.clone()).unwrap_or(TrustLevel::Untrusted)
    }
    pub fn verify_signature(&self, fingerprint: &str, _data: &[u8], _sig: &[u8]) -> bool {
        matches!(self.get_trust(fingerprint), TrustLevel::Full | TrustLevel::Ultimate)
    }
    pub fn key_count(&self) -> usize { self.keys.len() }
}
impl Default for TrustStore { fn default() -> Self { Self::new() } }
