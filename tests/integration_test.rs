use sigmaos_keyring::{Config, KeyringComponent, TrustKey, TrustLevel, TrustStore, Component};

#[test]
fn test_config_default_aaa() {
    let c = Config::default();
    assert!(c.enabled);
    assert!(!c.keyring_path.is_empty());
}

#[test]
fn test_trust_store_untrusted_default_aaa() {
    let store = TrustStore::new();
    assert_eq!(store.get_trust("AAAA1234"), TrustLevel::Untrusted);
}

#[test]
fn test_trust_store_add_key_aaa() {
    let mut store = TrustStore::new();
    store.add_key(TrustKey::new("BBBB5678", "SigmaOS Official", TrustLevel::Full));
    assert_eq!(store.get_trust("BBBB5678"), TrustLevel::Full);
    assert_eq!(store.key_count(), 1);
}

#[test]
fn test_trust_store_verify_signature_aaa() {
    let mut store = TrustStore::new();
    store.add_key(TrustKey::new("FP001", "SigmaOS", TrustLevel::Ultimate));
    assert!(store.verify_signature("FP001", b"data", b"sig"));
    assert!(!store.verify_signature("UNKNOWN", b"data", b"sig"));
}

#[test]
fn test_trust_store_marginal_no_verify_aaa() {
    let mut store = TrustStore::new();
    store.add_key(TrustKey::new("FP002", "Third Party", TrustLevel::Marginal));
    assert!(!store.verify_signature("FP002", b"data", b"sig"));
}

#[test]
fn test_component_lifecycle_aaa() {
    let mut c = KeyringComponent::new(Config::default());
    assert!(c.initialize().is_ok());
    assert!(c.health_check().is_ok());
    assert!(c.shutdown().is_ok());
}

#[test]
fn test_component_name_aaa() {
    let c = KeyringComponent::new(Config::default());
    assert_eq!(c.name(), "sigmaos-keyring");
}
