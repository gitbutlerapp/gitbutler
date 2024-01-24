pub enum Authorization {
    Basic {
        pub username: String,
        pub password: String,
    },
    PublicKey {
        pub path: PathBuf
    }
}
