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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ownership() {
        let ownership: OwnershipClaim = "foo/bar.rs:1-2,4-5".parse().unwrap();
        assert_eq!(
            ownership,
            OwnershipClaim {
                file_path: "foo/bar.rs".into(),
                hunks: vec![(1..=2).into(), (4..=5).into()]
            }
        );
    }

    #[test]
    fn parse_ownership_tricky_file_name() {
        assert_eq!("file:name:1-2,4-5".parse::<OwnershipClaim>().unwrap(), {
            OwnershipClaim {
                file_path: "file:name".into(),
                hunks: vec![(1..=2).into(), (4..=5).into()],
            }
        });
    }

    #[test]
    fn parse_ownership_no_ranges() {
        "foo/bar.rs".parse::<OwnershipClaim>().unwrap_err();
    }

    #[test]
    fn ownership_to_from_string() {
        let ownership = OwnershipClaim {
            file_path: "foo/bar.rs".into(),
            hunks: vec![(1..=2).into(), (4..=5).into()],
        };
        assert_eq!(ownership.to_string(), "foo/bar.rs:1-2,4-5".to_string());
        assert_eq!(
            ownership.to_string().parse::<OwnershipClaim>().unwrap(),
            ownership
        );
    }

    #[test]
    fn test_plus() {
        vec![
            ("file.txt:1-10", "another.txt:1-5", "file.txt:1-10"),
            ("file.txt:1-10,3-14", "file.txt:3-14", "file.txt:3-14,1-10"),
            ("file.txt:5-10", "file.txt:1-5", "file.txt:1-5,5-10"),
            ("file.txt:1-10", "file.txt:1-5", "file.txt:1-5,1-10"),
            ("file.txt:1-5,2-2", "file.txt:1-10", "file.txt:1-10,1-5,2-2"),
            (
                "file.txt:1-10",
                "file.txt:8-15,20-25",
                "file.txt:20-25,8-15,1-10",
            ),
            ("file.txt:1-10", "file.txt:1-10", "file.txt:1-10"),
            ("file.txt:1-10,3-15", "file.txt:1-10", "file.txt:1-10,3-15"),
        ]
        .into_iter()
        .map(|(a, b, expected)| {
            (
                a.parse::<OwnershipClaim>().unwrap(),
                b.parse::<OwnershipClaim>().unwrap(),
                expected.parse::<OwnershipClaim>().unwrap(),
            )
        })
        .for_each(|(a, b, expected)| {
            let got = a.plus(&b);
            assert_eq!(
                got, expected,
                "{} plus {}, expected {}, got {}",
                a, b, expected, got
            );
        });
    }

    #[test]
    fn test_minus() {
        vec![
            (
                "file.txt:1-10",
                "another.txt:1-5",
                (None, Some("file.txt:1-10")),
            ),
            (
                "file.txt:1-10",
                "file.txt:1-5",
                (None, Some("file.txt:1-10")),
            ),
            (
                "file.txt:1-10",
                "file.txt:11-15",
                (None, Some("file.txt:1-10")),
            ),
            (
                "file.txt:1-10",
                "file.txt:1-10",
                (Some("file.txt:1-10"), None),
            ),
            (
                "file.txt:1-10,11-15",
                "file.txt:11-15",
                (Some("file.txt:11-15"), Some("file.txt:1-10")),
            ),
            (
                "file.txt:1-10,11-15,15-17",
                "file.txt:1-10,15-17",
                (Some("file.txt:1-10,15-17"), Some("file.txt:11-15")),
            ),
        ]
        .into_iter()
        .map(|(a, b, expected)| {
            (
                a.parse::<OwnershipClaim>().unwrap(),
                b.parse::<OwnershipClaim>().unwrap(),
                (
                    expected.0.map(|s| s.parse::<OwnershipClaim>().unwrap()),
                    expected.1.map(|s| s.parse::<OwnershipClaim>().unwrap()),
                ),
            )
        })
        .for_each(|(a, b, expected)| {
            let got = a.minus(&b);
            assert_eq!(
                got, expected,
                "{} minus {}, expected {:?}, got {:?}",
                a, b, expected, got
            );
        });
    }

    #[test]
    fn test_equal() {
        vec![
            ("file.txt:1-10", "file.txt:1-10", true),
            ("file.txt:1-10", "file.txt:1-11", false),
            ("file.txt:1-10,11-15", "file.txt:11-15,1-10", false),
            ("file.txt:1-10,11-15", "file.txt:1-10,11-15", true),
        ]
        .into_iter()
        .map(|(a, b, expected)| {
            (
                a.parse::<OwnershipClaim>().unwrap(),
                b.parse::<OwnershipClaim>().unwrap(),
                expected,
            )
        })
        .for_each(|(a, b, expected)| {
            assert_eq!(a == b, expected, "{} == {}, expected {}", a, b, expected);
        });
    }
}
