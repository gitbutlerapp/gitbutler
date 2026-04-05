use std::path::Path;

fn main() {
    // When the embedded-frontend feature is active, rust-embed requires the
    // dist directory to exist at compile time. Create a placeholder if the
    // frontend hasn't been built yet so the crate compiles without running
    // `pnpm build` first.
    if std::env::var("CARGO_FEATURE_EMBEDDED_FRONTEND").is_ok() {
        let dist = Path::new("../../apps/desktop/build");
        if !dist.exists() {
            std::fs::create_dir_all(dist).expect("failed to create dist placeholder");
            std::fs::write(
                dist.join("index.html"),
                "<html><body><p>Frontend not built. Run: \
                 <code>pnpm --filter @gitbutler/desktop build</code> \
                 (outputs to apps/desktop/build/)</p></body></html>",
            )
            .expect("failed to write placeholder index.html");
        }
        // Emit rerun-if-changed for every file in dist so this build script
        // is re-run whenever `pnpm build` updates any asset.
        emit_rerun_if_changed_recursive(dist);

        // Compute a hash of the entire dist tree and emit it as a rustc-env
        // variable. When the hash changes, Cargo recompiles the crate, which
        // causes rust-embed to re-embed the updated files.
        let hash = dir_hash(dist);
        println!("cargo:rustc-env=EMBEDDED_FRONTEND_HASH={hash}");
    }
}

/// A fast, order-independent hash of every file path + contents under `dir`.
fn dir_hash(dir: &Path) -> u64 {
    use std::collections::BTreeMap;
    use std::hash::Hash as _;

    // Collect path → contents into a sorted map so the hash is stable
    // regardless of directory traversal order.
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    collect_files(dir, dir, &mut files);

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    files.hash(&mut hasher);
    std::hash::Hasher::finish(&hasher)
}

fn collect_files(root: &Path, dir: &Path, out: &mut std::collections::BTreeMap<String, Vec<u8>>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, out);
        } else {
            let key = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();
            let contents = std::fs::read(&path).unwrap_or_default();
            out.insert(key, contents);
        }
    }
}

fn emit_rerun_if_changed_recursive(dir: &Path) {
    println!("cargo:rerun-if-changed={}", dir.display());
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                emit_rerun_if_changed_recursive(&path);
            } else {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
}
