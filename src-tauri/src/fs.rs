use std::{fs::read_dir, path::Path};

// return a list of files in directory recursively
pub fn list_files(path: &Path) -> Result<Vec<String>, std::io::Error> {
    let mut files = Vec::new();
    if path.is_dir() {
        for entry in read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                files.append(&mut list_files(&path)?);
            } else {
                files.push(path.to_str().unwrap().to_string());
            }
        }
    }
    files.sort();
    Ok(files)
}
