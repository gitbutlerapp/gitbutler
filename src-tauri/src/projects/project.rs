use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Project {
    pub id: String,
    pub title: String,
    pub path: String,
}

impl AsRef<Project> for Project {
    fn as_ref(&self) -> &Project {
        self
    }
}

impl Project {
    pub fn from_path(path: String) -> Result<Self, Error> {
        // make sure path exists
        let path = std::path::Path::new(&path);
        if !path.exists() {
            return Err(Error::Message("Path does not exist".to_string()));
        }

        // make sure path is a directory
        if !path.is_dir() {
            return Err(Error::Message("Path is not a directory".to_string()));
        }

        // make sure it's a git repository
        if !path.join(".git").exists() {
            return Err(Error::Message("Path is not a git repository".to_string()));
        };

        // title is the base name of the file
        path.into_iter()
            .last()
            .map(|p| p.to_str().unwrap().to_string())
            .map(|title| Self {
                id: uuid::Uuid::new_v4().to_string(),
                title,
                path: path.to_str().unwrap().to_string(),
            })
            .ok_or(Error::Message("Could not get title".to_string()))
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum Error {
    Message(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Message(msg) => write!(f, "Error: {}", msg),
        }
    }
}
