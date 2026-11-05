use crate::trust::TrustLevel;

/// Unique identifier for a key (wraps a u64 for type safety).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyId(pub u64);

/// Represents a cryptographic key stored in the keyring.
#[derive(Debug, Clone)]
pub struct Key {
    /// Unique identifier for this key.
    pub id: KeyId,
    /// 20-byte SHA-1 fingerprint of the key.
    pub fingerprint: [u8; 20],
    /// Human-readable owner name or email.
    pub owner: String,
    /// Trust level assigned to this key.
    pub trust: TrustLevel,
    /// Optional expiry timestamp (Unix seconds). `None` means no expiry.
    pub expires: Option<u64>,
}

impl Key {
    /// Creates a new [`Key`] with the given fields.
    pub fn new(
        id: KeyId,
        fingerprint: [u8; 20],
        owner: impl Into<String>,
        trust: TrustLevel,
        expires: Option<u64>,
    ) -> Self {
        Self {
            id,
            fingerprint,
            owner: owner.into(),
            trust,
            expires,
        }
    }

    /// Returns `true` if the key has expired relative to `now_secs`.
    pub fn is_expired(&self, now_secs: u64) -> bool {
        self.expires.map(|exp| now_secs >= exp).unwrap_or(false)
    }

    /// Returns the fingerprint as a lowercase hex string.
    pub fn fingerprint_hex(&self) -> String {
        self.fingerprint
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trust::TrustLevel;

    fn sample_fingerprint(byte: u8) -> [u8; 20] {
        [byte; 20]
    }

    // AAA: Arrange a key with an expiry, Act on is_expired, Assert result.
    #[test]
    fn test_is_expired_when_past_expiry() {
        // Arrange
        let key = Key::new(KeyId(1), sample_fingerprint(0xAB), "alice", TrustLevel::Full, Some(1000));
        // Act
        let expired = key.is_expired(1001);
        // Assert
        assert!(expired);
    }

    // AAA: Arrange a key with an expiry, Act before expiry, Assert not expired.
    #[test]
    fn test_is_not_expired_before_expiry() {
        // Arrange
        let key = Key::new(KeyId(2), sample_fingerprint(0xCD), "bob", TrustLevel::Marginal, Some(5000));
        // Act
        let expired = key.is_expired(4999);
        // Assert
        assert!(!expired);
    }

    // AAA: Arrange a key with no expiry, Act, Assert never expired.
    #[test]
    fn test_no_expiry_never_expired() {
        // Arrange
        let key = Key::new(KeyId(3), sample_fingerprint(0x01), "carol", TrustLevel::Ultimate, None);
        // Act & Assert
        assert!(!key.is_expired(u64::MAX));
    }

    // AAA: Arrange a key, Act fingerprint_hex, Assert correct length and lowercase hex.
    #[test]
    fn test_fingerprint_hex_format() {
        // Arrange
        let mut fp = [0u8; 20];
        fp[0] = 0xDE;
        fp[19] = 0xAD;
        let key = Key::new(KeyId(4), fp, "dave", TrustLevel::Unknown, None);
        // Act
        let hex = key.fingerprint_hex();
        // Assert
        assert_eq!(hex.len(), 40);
        assert!(hex.starts_with("de"));
        assert!(hex.ends_with("ad"));
    }
}
