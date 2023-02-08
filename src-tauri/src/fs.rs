use std::{fs, path::Path};

fn list_files_abs(dir_path: &Path) -> Result<Vec<String>, std::io::Error> {
    let mut files = Vec::new();
    if dir_path.is_dir() {
        for entry in fs::read_dir(dir_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let mut sub_files = list_files(&path)?;
                files.append(&mut sub_files);
            } else {
                match path.to_str() {
                    Some(path) => files.push(path.to_string()),
                    None => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            "Invalid path",
                        ))
                    }
                }
            }
        }
    }
    files.sort();
    Ok(files)
}

// Returns an ordered list of relative paths for files inside a directory recursively.
pub fn list_files(dir_path: &Path) -> Result<Vec<String>, std::io::Error> {
    list_files_abs(dir_path).map(|files| {
        files
            .iter()
            .filter_map(|file| {
                let file_path = Path::new(file);
                let relative_path = file_path.strip_prefix(dir_path).ok()?;
                relative_path.to_str().map(|s| s.to_string())
            })
            .collect()
    })
}
