#![cfg(windows)]

use std::time::{SystemTime, UNIX_EPOCH};
use story_provider::{ProviderCredentialId, ProviderCredentialStore, WindowsCredentialStore};

#[test]
#[ignore = "mutates the current user's Windows Credential Manager"]
fn real_windows_credential_round_trip_and_cleanup() {
    let unique = format!(
        "smoke-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    let id = ProviderCredentialId::new("integration-test", unique).unwrap();
    let store = WindowsCredentialStore::new();

    struct Cleanup<'a> {
        store: &'a WindowsCredentialStore,
        id: &'a ProviderCredentialId,
    }
    impl Drop for Cleanup<'_> {
        fn drop(&mut self) {
            let _ = self.store.delete(self.id);
        }
    }
    let _cleanup = Cleanup {
        store: &store,
        id: &id,
    };

    assert!(store.get(&id).unwrap().is_none());
    store.set(&id, b"first-test-secret").unwrap();
    assert_eq!(
        store.get(&id).unwrap().unwrap().expose_secret(),
        b"first-test-secret"
    );

    store.set(&id, b"replacement-test-secret").unwrap();
    assert_eq!(
        store.get(&id).unwrap().unwrap().expose_secret(),
        b"replacement-test-secret"
    );
    assert!(store.delete(&id).unwrap());
    assert!(!store.delete(&id).unwrap());
}
