use std::fmt;

use super::FileOwnership;

#[derive(Debug, PartialEq, Clone, Default)]
pub struct Ownership {
    pub files: Vec<FileOwnership>,
}

impl fmt::Display for Ownership {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for file in &self.files {
            writeln!(f, "{}", file)?;
        }
        Ok(())
    }
}

impl TryFrom<&str> for Ownership {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        let mut ownership = Ownership::default();
        for line in s.lines() {
            ownership.put(&FileOwnership::try_from(line)?);
        }
        Ok(ownership)
    }
}

impl Ownership {
    pub fn put(&mut self, ownership: &FileOwnership) {
        let target = self
            .files
            .iter()
            .cloned()
            .find(|o| o.file_path == ownership.file_path);

        self.files.retain(|o| o.file_path != ownership.file_path);

        if let Some(target) = target {
            self.files.push(target.plus(ownership));
        } else {
            self.files.push(ownership.clone());
        }

        self.files.sort_by(|a, b| a.file_path.cmp(&b.file_path));
        self.files.dedup();
    }

    pub fn take(&mut self, ownership: &FileOwnership) {
        let target = self
            .files
            .iter()
            .cloned()
            .find(|o| o.file_path == ownership.file_path);

        self.files.retain(|o| o.file_path != ownership.file_path);

        if let Some(target) = target.as_ref() {
            if let Some(remaining) = target.minus(ownership) {
                self.files.push(remaining);
                self.files.sort_by(|a, b| a.file_path.cmp(&b.file_path));
                self.files.dedup();
            }
        }
    }

    pub fn explicitly_owns(&self, file: &FileOwnership) -> bool {
        self.files
            .iter()
            .filter(|ownership| !ownership.is_full()) // only consider explicit ownership
            .any(|ownership| ownership.contains(file))
    }

    pub fn owns_by_proximity(&self, file: &FileOwnership) -> bool {
        self.files
            .iter()
            .filter(|file_ownership| !file_ownership.is_full()) // only consider explicit ownership
            .any(|file_ownership| {
                file_ownership.hunks.iter().any(|range| {
                    file.hunks
                        .iter()
                        .any(|r| r.touches(range) || r.intersects(range))
                })
            })
    }

    pub fn implicitly_owns(&self, file: &FileOwnership) -> bool {
        self.files.iter().any(|ownership| ownership.contains(file))
    }
}
