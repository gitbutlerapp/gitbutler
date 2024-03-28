#[cfg(not(target_os = "windows"))]
mod not_windows {
    use gitbutler_app::keys::storage::Storage;
    use gitbutler_app::keys::Controller;
    use std::fs;
    #[cfg(target_family = "unix")]
    use std::os::unix::prelude::*;

    use crate::Suite;

    #[test]
    fn test_get_or_create() {
        let suite = Suite::default();
        let controller = Controller::new(Storage::from_path(&suite.local_app_data));

        let once = controller.get_or_create().unwrap();
        let twice = controller.get_or_create().unwrap();
        assert_eq!(once, twice);

        // check permissions of the private key
        let permissions = fs::metadata(suite.local_app_data.join("keys/ed25519"))
            .unwrap()
            .permissions();
        let perms = format!("{:o}", permissions.mode());
        assert_eq!(perms, "100600");
    }
}
