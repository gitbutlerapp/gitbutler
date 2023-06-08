use std::{collections::HashMap, path};

use anyhow::Result;

use crate::reader;

use super::Delta;

pub struct DeltasReader<'reader> {
    reader: &'reader dyn reader::Reader,
}

impl<'reader> DeltasReader<'reader> {
    pub fn new(reader: &'reader dyn reader::Reader) -> Self {
        DeltasReader { reader }
    }

    pub fn read_file<P: AsRef<std::path::Path>>(&self, path: P) -> Result<Option<Vec<Delta>>> {
        let path = path.as_ref();
        let file_deltas_path = std::path::Path::new("session/deltas").join(path);
        match self.reader.read_string(file_deltas_path.to_str().unwrap()) {
            Ok(content) => {
                if content.is_empty() {
                    // this is a leftover from some bug, shouldn't happen anymore
                    Ok(None)
                } else {
                    Ok(Some(serde_json::from_str(&content)?))
                }
            }
            Err(reader::Error::NotFound) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    pub fn read_virtual_file<P: AsRef<std::path::Path>>(
        &self,
        vbranch_id: &str,
        path: P,
    ) -> Result<Option<Vec<Delta>>> {
        let path = path.as_ref();
        let file_deltas_path =
            std::path::Path::new(&format!("branches/{}/deltas", vbranch_id)).join(path);
        match self.reader.read_string(file_deltas_path.to_str().unwrap()) {
            Ok(content) => Ok(Some(serde_json::from_str(&content)?)),
            Err(reader::Error::NotFound) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    pub fn read(&self, paths: Option<Vec<&str>>) -> Result<HashMap<String, Vec<Delta>>> {
        let deltas_dir = path::Path::new("session/deltas");
        let files = self.reader.list_files(deltas_dir.to_str().unwrap())?;
        let mut result = HashMap::new();
        for file_path in files {
            if let Some(paths) = paths.as_ref() {
                if !paths.iter().any(|path| file_path.eq(path)) {
                    continue;
                }
            }
            if let Some(deltas) = self.read_file(file_path.clone())? {
                result.insert(file_path, deltas);
            }
        }
        Ok(result)
    }
}
