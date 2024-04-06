use gitbutler_core::keys::{PrivateKey, PublicKey};

mod controller {
    #[cfg(not(target_os = "windows"))]
    mod not_windows {
        use std::fs;
        #[cfg(target_family = "unix")]
        use std::os::unix::prelude::*;

        use gitbutler_core::keys::{storage::Storage, Controller};

        use gitbutler_testsupport::Suite;

        #[test]
        fn get_or_create() {
            let suite = Suite::default();
            let controller = Controller::new(Storage::from_path(suite.local_app_data()));

            let once = controller.get_or_create().unwrap();
            let twice = controller.get_or_create().unwrap();
            assert_eq!(once, twice);

            // check permissions of the private key
            let permissions = fs::metadata(suite.local_app_data().join("keys/ed25519"))
                .unwrap()
                .permissions();
            let perms = format!("{:o}", permissions.mode());
            assert_eq!(perms, "100600");
        }
    }
}

#[test]
fn to_from_string_private() {
    let private_key = PrivateKey::generate();
    let serialized = private_key.to_string();
    let deserialized: PrivateKey = serialized.parse().unwrap();
    assert_eq!(private_key, deserialized);
}

#[test]
fn to_from_string_public() {
    let private_key = PrivateKey::generate();
    let public_key = private_key.public_key();
    let serialized = public_key.to_string();
    let deserialized: PublicKey = serialized.parse().unwrap();
    assert_eq!(public_key, deserialized);
}

#[test]
fn serde_private() {
    let private_key = PrivateKey::generate();
    let serialized = serde_json::to_string(&private_key).unwrap();
    let deserialized: PrivateKey = serde_json::from_str(&serialized).unwrap();
    assert_eq!(private_key, deserialized);
}

#[test]
fn serde_public() {
    let private_key = PrivateKey::generate();
    let public_key = private_key.public_key();
    let serialized = serde_json::to_string(&public_key).unwrap();
    let deserialized: PublicKey = serde_json::from_str(&serialized).unwrap();
    assert_eq!(public_key, deserialized);
}
