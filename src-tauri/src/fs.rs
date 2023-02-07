use std::{fs::read_dir, path::Path};

// return a list of files in directory recursively
pub fn list_files(path: &Path) -> Vec<String> {
    let mut files = Vec::new();
    if path.is_dir() {
        for entry in read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                files.append(&mut list_files(&path));
            } else {
                files.push(path.to_str().unwrap().to_string());
            }
        }
    }
    files.sort();
    files
}
