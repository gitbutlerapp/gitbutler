//! Provides all of the types and implementations
//! necessary to manage the hunk split database.

use anyhow::{Context, Result};
use std::{
    collections::{hash_map::Entry, HashMap},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use super::{branch::HunkHash, BranchId};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Splits {
    split: HashMap<HunkHash, SplitEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct SplitEntry {
    pub ownership: Vec<BranchId>,
}

#[derive(Debug, Clone)]
pub struct SplitDatabase {
    path: PathBuf,
    splits: Splits,
}

impl SplitDatabase {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            splits: Splits::default(),
            path: path.as_ref().to_path_buf(),
        }
    }

    pub fn load(&mut self) -> Result<()> {
        let file = match std::fs::File::open(&self.path) {
            Ok(file) => file,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    return Ok(());
                }
                return Err(e).context("failed to open split database file");
            }
        };
        let reader = std::io::BufReader::new(file);
        self.splits = serde_json::from_reader(reader).context("failed to read split database")?;
        Ok(())
    }

    pub fn save(&self) -> Result<()> {
        let file =
            std::fs::File::create(&self.path).context("failed to create split database file")?;
        serde_json::to_writer_pretty(file, &self.splits)
            .context("failed to write split database")?;
        Ok(())
    }

    pub fn get(&self, hash: &HunkHash) -> Option<&SplitEntry> {
        self.splits.split.get(hash)
    }

    pub fn get_mut(&mut self, hash: &HunkHash) -> Option<&mut SplitEntry> {
        self.splits.split.get_mut(hash)
    }

    pub fn get_mut_or_insert(&mut self, hash: HunkHash) -> &mut SplitEntry {
        if let Entry::Vacant(e) = self.splits.split.entry(hash) {
            e.insert(SplitEntry::default());
        }

        self.splits.split.get_mut(&hash).unwrap()
    }

    pub fn set(&mut self, hash: HunkHash, entry: SplitEntry) -> Option<SplitEntry> {
        self.splits.split.insert(hash, entry)
    }

    pub fn splits(&self) -> &Splits {
        &self.splits
    }

    pub fn splits_mut(&mut self) -> &mut Splits {
        &mut self.splits
    }

    pub fn replace_splits(&mut self, splits: Splits) -> Splits {
        std::mem::replace(&mut self.splits, splits)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}
