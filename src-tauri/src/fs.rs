use std::path::Path;

use walkdir::WalkDir;

// Returns an ordered list of relative paths for files inside a directory recursively.
pub fn list_files(dir_path: &Path) -> Result<Vec<String>, std::io::Error> {
    let mut files = vec![];
    for entry in WalkDir::new(dir_path) {
        let entry = entry?;
        if entry.file_type().is_file() {
            let path = entry.path();
            let path = path.strip_prefix(dir_path).unwrap();
            let path = path.to_str().unwrap().to_string();
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}
