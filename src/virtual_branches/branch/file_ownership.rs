use std::{fmt, path, str::FromStr, vec};

use anyhow::{Context, Result};

use super::hunk::Hunk;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct OwnershipClaim {
    pub file_path: path::PathBuf,
    pub hunks: Vec<Hunk>,
}

impl FromStr for OwnershipClaim {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        let mut file_path_parts = vec![];
        let mut ranges = vec![];
        for part in value.split(':').rev() {
            match part
                .split(',')
                .map(str::parse)
                .collect::<Result<Vec<Hunk>>>()
            {
                Ok(rr) => ranges.extend(rr),
                Err(_) => {
                    file_path_parts.insert(0, part);
                }
            }
        }

        if ranges.is_empty() {
            Err(anyhow::anyhow!("ownership ranges cannot be empty"))
        } else {
            Ok(Self {
                file_path: file_path_parts
                    .join(":")
                    .parse()
                    .context(format!("failed to parse file path from {}", value))?,
                hunks: ranges.clone(),
            })
        }
    }
}

impl OwnershipClaim {
    pub fn is_full(&self) -> bool {
        self.hunks.is_empty()
    }

    pub fn contains(&self, another: &OwnershipClaim) -> bool {
        if !self.file_path.eq(&another.file_path) {
            return false;
        }

        if self.hunks.is_empty() {
            // full ownership contains any partial ownership
            return true;
        }

        if another.hunks.is_empty() {
            // partial ownership contains no full ownership
            return false;
        }

        another.hunks.iter().all(|hunk| self.hunks.contains(hunk))
    }

    // return a copy of self, with another ranges added
    pub fn plus(&self, another: &OwnershipClaim) -> OwnershipClaim {
        if !self.file_path.eq(&another.file_path) {
            return self.clone();
        }

        if self.hunks.is_empty() {
            // full ownership + partial ownership = full ownership
            return self.clone();
        }

        if another.hunks.is_empty() {
            // partial ownership + full ownership = full ownership
            return another.clone();
        }

        let mut hunks = self
            .hunks
            .iter()
            .filter(|hunk| !another.hunks.contains(hunk))
            .cloned()
            .collect::<Vec<Hunk>>();

        another.hunks.iter().for_each(|hunk| {
            hunks.insert(0, hunk.clone());
        });

        OwnershipClaim {
            file_path: self.file_path.clone(),
            hunks,
        }
    }

    // returns (taken, remaining)
    // if all of the ranges are removed, return None
    pub fn minus(
        &self,
        another: &OwnershipClaim,
    ) -> (Option<OwnershipClaim>, Option<OwnershipClaim>) {
        if !self.file_path.eq(&another.file_path) {
            // no changes
            return (None, Some(self.clone()));
        }

        if another.hunks.is_empty() {
            // any ownership - full ownership = empty ownership
            return (Some(self.clone()), None);
        }

        if self.hunks.is_empty() {
            // full ownership - partial ownership = full ownership, since we don't know all the
            // hunks.
            return (None, Some(self.clone()));
        }

        let mut left = self.hunks.clone();
        let mut taken = vec![];
        for range in &another.hunks {
            left = left
                .iter()
                .flat_map(|r: &Hunk| -> Vec<Hunk> {
                    if r.eq(range) {
                        taken.push(r.clone());
                        vec![]
                    } else {
                        vec![r.clone()]
                    }
                })
                .collect();
        }

        (
            if taken.is_empty() {
                None
            } else {
                Some(OwnershipClaim {
                    file_path: self.file_path.clone(),
                    hunks: taken,
                })
            },
            if left.is_empty() {
                None
            } else {
                Some(OwnershipClaim {
                    file_path: self.file_path.clone(),
                    hunks: left,
                })
            },
        )
    }
}

impl fmt::Display for OwnershipClaim {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        if self.hunks.is_empty() {
            write!(f, "{}", self.file_path.display())
        } else {
            write!(
                f,
                "{}:{}",
                self.file_path.display(),
                self.hunks
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<String>>()
                    .join(",")
            )
        }
    }
}
