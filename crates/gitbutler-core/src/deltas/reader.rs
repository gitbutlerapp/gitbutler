use std::{collections::HashMap, path};

use anyhow::{Context, Result};

use super::Delta;
use crate::{reader, sessions};

pub struct DeltasReader<'reader> {
    reader: &'reader reader::Reader<'reader>,
}

impl<'reader> From<&'reader reader::Reader<'reader>> for DeltasReader<'reader> {
    fn from(reader: &'reader reader::Reader<'reader>) -> Self {
        DeltasReader { reader }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ReadError {
    #[error("not found")]
    NotFound,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl<'reader> DeltasReader<'reader> {
    pub fn new(reader: &'reader sessions::Reader<'reader>) -> Self {
        DeltasReader {
            reader: reader.reader(),
        }
    }

    pub fn read_file<P: AsRef<std::path::Path>>(&self, path: P) -> Result<Option<Vec<Delta>>> {
        match self.read(Some(&[path.as_ref()])) {
            Ok(deltas) => Ok(deltas.into_iter().next().map(|(_, deltas)| deltas)),
            Err(ReadError::NotFound) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    pub fn read(
        &self,
        filter: Option<&[&path::Path]>,
    ) -> Result<HashMap<path::PathBuf, Vec<Delta>>, ReadError> {
        let deltas_dir = path::Path::new("session/deltas");
        let mut paths = self.reader.list_files(deltas_dir)?;
        if let Some(filter) = filter {
            paths = paths
                .into_iter()
                .filter(|file_path| filter.iter().any(|path| file_path.eq(path)))
                .collect::<Vec<_>>();
        }
        paths = paths.iter().map(|path| deltas_dir.join(path)).collect();
        let files = self.reader.batch(&paths).context("failed to batch read")?;

        let files = files
            .into_iter()
            .map(|file| {
                file.map_err(|error| match error {
                    reader::Error::NotFound => ReadError::NotFound,
                    error => ReadError::Other(error.into()),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(paths
            .into_iter()
            .zip(files)
            .filter_map(|(path, file)| {
                path.strip_prefix(deltas_dir)
                    .ok()
                    .map(|path| (path.to_path_buf(), file))
            })
            .filter_map(|(path, file)| {
                if let reader::Content::UTF8(content) = file {
                    if content.is_empty() {
                        // this is a leftover from some bug, shouldn't happen anymore
                        return None;
                    }
                    let deltas = serde_json::from_str(&content).ok()?;
                    Some(Ok((path, deltas)))
                } else {
                    Some(Err(anyhow::anyhow!("unexpected content type")))
                }
            })
            .collect::<Result<HashMap<_, _>>>()?)
    }
}
