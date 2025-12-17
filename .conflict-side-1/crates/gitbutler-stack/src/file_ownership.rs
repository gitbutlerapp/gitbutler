use std::{fmt, path, path::Path, str::FromStr, vec};

use anyhow::{Context as _, Result};
use but_meta::virtual_branches_legacy_types;
use gitbutler_diff::Hunk;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct OwnershipClaim {
    pub file_path: path::PathBuf,
    pub hunks: Vec<Hunk>,
}

impl From<virtual_branches_legacy_types::OwnershipClaim> for OwnershipClaim {
    fn from(
        virtual_branches_legacy_types::OwnershipClaim { file_path, hunks }: virtual_branches_legacy_types::OwnershipClaim,
    ) -> Self {
        OwnershipClaim {
            file_path,
            hunks: hunks.into_iter().map(hunk_to_hunk_mirror).collect(),
        }
    }
}

impl From<OwnershipClaim> for virtual_branches_legacy_types::OwnershipClaim {
    fn from(OwnershipClaim { file_path, hunks }: OwnershipClaim) -> Self {
        virtual_branches_legacy_types::OwnershipClaim {
            file_path,
            hunks: hunks.into_iter().map(mirror_hunk_to_hunk).collect(),
        }
    }
}

fn hunk_to_hunk_mirror(
    virtual_branches_legacy_types::Hunk {
        hash,
        start,
        end,
        hunk_header,
    }: virtual_branches_legacy_types::Hunk,
) -> Hunk {
    Hunk {
        hash,
        start,
        end,
        hunk_header: hunk_header.map(header_to_header_mirror),
    }
}

fn mirror_hunk_to_hunk(
    Hunk {
        hash,
        start,
        end,
        hunk_header,
    }: Hunk,
) -> virtual_branches_legacy_types::Hunk {
    virtual_branches_legacy_types::Hunk {
        hash,
        start,
        end,
        hunk_header: hunk_header.map(mirror_header_to_header),
    }
}

fn header_to_header_mirror(
    virtual_branches_legacy_types::HunkHeader {
        old_start,
        old_lines,
        new_start,
        new_lines,
    }: virtual_branches_legacy_types::HunkHeader,
) -> gitbutler_diff::HunkHeader {
    gitbutler_diff::HunkHeader {
        old_start,
        old_lines,
        new_start,
        new_lines,
    }
}

fn mirror_header_to_header(
    gitbutler_diff::HunkHeader {
        old_start,
        old_lines,
        new_start,
        new_lines,
    }: gitbutler_diff::HunkHeader,
) -> virtual_branches_legacy_types::HunkHeader {
    virtual_branches_legacy_types::HunkHeader {
        old_start,
        old_lines,
        new_start,
        new_lines,
    }
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
                    .context(format!("failed to parse file path from {value}"))?,
                hunks: ranges.clone(),
            })
        }
    }
}

impl<'a> From<&'a OwnershipClaim> for (&'a Path, &'a [Hunk]) {
    fn from(value: &'a OwnershipClaim) -> Self {
        (&value.file_path, &value.hunks)
    }
}

impl OwnershipClaim {
    pub(crate) fn is_full(&self) -> bool {
        self.hunks.is_empty()
    }

    // return a copy of self, with another ranges added
    pub fn plus(&self, another: OwnershipClaim) -> OwnershipClaim {
        if self.file_path != another.file_path {
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

        for hunk in another.hunks {
            hunks.insert(0, hunk);
        }

        OwnershipClaim {
            file_path: another.file_path,
            hunks,
        }
    }

    /// returns `(taken, remaining)` if all the ranges are removed, return `None`
    pub fn minus(
        &self,
        another: &OwnershipClaim,
    ) -> (Option<OwnershipClaim>, Option<OwnershipClaim>) {
        if self.file_path != another.file_path {
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
