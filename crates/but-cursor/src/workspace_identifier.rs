use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
struct Uri {
    pub scheme: String,
    #[serde(rename = "fsPath")]
    pub fs_path: String,
}

impl Uri {
    fn from_file_path<P: AsRef<Path>>(path: P) -> Self {
        Self {
            scheme: "file".to_string(),
            fs_path: path.as_ref().to_string_lossy().to_string(),
        }
    }
}

/// Length of workspace identifiers that are not empty. Those are
/// MD5 hashes (128bits / 4 due to hex presentation).
pub const NON_EMPTY_WORKSPACE_ID_LENGTH: usize = 128 / 4;

/// Get single folder workspace identifier
pub fn get_single_folder_workspace_identifier<P: AsRef<Path>>(path: P) -> anyhow::Result<String> {
    let folder_uri = &Uri::from_file_path(path);

    fn get_folder_id(folder_uri: &Uri) -> anyhow::Result<String> {
        // Remote: produce a hash from the entire URI
        if folder_uri.scheme != "file" {
            let uri_string = format!("{}:{}", folder_uri.scheme, folder_uri.fs_path);
            let hash = md5::compute(uri_string.as_bytes());
            return Ok(format!("{hash:x}"));
        }

        // Local: we use the ctime as extra salt to the
        // identifier so that folders getting recreated
        // result in a different identifier. However, if
        // the stat is not provided we return `undefined`
        // to ensure identifiers are stable for the given
        // URI.

        let path = Path::new(&folder_uri.fs_path);
        let metadata = std::fs::metadata(path)?;

        let ctime: Option<i64> = if cfg!(target_os = "linux") {
            // Linux: birthtime is ctime, so we cannot use it! We use the ino instead!
            #[cfg(target_os = "linux")]
            {
                use std::os::unix::fs::MetadataExt;
                Some(metadata.ino() as i64)
            }
            #[cfg(not(target_os = "linux"))]
            None
        } else if cfg!(target_os = "macos") {
            // macOS: birthtime is fine to use as is
            #[cfg(target_os = "macos")]
            {
                use std::os::macos::fs::MetadataExt;
                // Node.js uses getTime() which returns milliseconds, so we need to convert from seconds to milliseconds
                // Also need to account for nanoseconds part with proper rounding
                let seconds = metadata.st_birthtime();
                let nanos = metadata.st_birthtime_nsec();
                let millis = seconds * 1000 + ((nanos + 500_000) / 1_000_000); // Round to nearest millisecond
                Some(millis)
            }
            #[cfg(not(target_os = "macos"))]
            None
        } else if cfg!(target_os = "windows") {
            // Windows: fix precision issue in node.js 8.x to get 7.x results
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::fs::MetadataExt;
                let birth_time = metadata.creation_time() as f64;
                Some(birth_time.floor() as i64)
            }
            #[cfg(not(target_os = "windows"))]
            None
        } else {
            None
        };

        let mut input = folder_uri.fs_path.clone();
        if let Some(ctime_val) = ctime {
            input.push_str(&ctime_val.to_string());
        }

        let hash = md5::compute(input.as_bytes());
        Ok(format!("{hash:x}"))
    }

    let folder_id = get_folder_id(folder_uri)?;
    Ok(folder_id)
}
