use std::fmt;

use serde::{Deserialize, Serialize, Serializer};

use super::FileOwnership;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ownership {
    pub files: Vec<FileOwnership>,
}

impl Serialize for Ownership {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.to_string().as_str())
    }
}

impl<'de> Deserialize<'de> for Ownership {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ownership::try_from(s.as_str()).map_err(serde::de::Error::custom)
    }
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
        if self.files.contains(ownership) {
            return;
        }

        if ownership.is_full() {
            self.files.push(ownership.clone());
        } else {
            let target = self
                .files
                .iter()
                .filter(|o| !o.is_full()) // only consider explicit ownership
                .cloned()
                .find(|o| o.file_path == ownership.file_path);

            self.files
                .retain(|o| o.is_full() || o.file_path != ownership.file_path);

            if let Some(target) = target {
                self.files.push(target.plus(ownership));
            } else {
                self.files.push(ownership.clone());
            }
        }
    }

    // modifies the ownership in-place and returns the file ownership that was taken, if any.
    pub fn take(&mut self, ownership: &FileOwnership) -> Vec<FileOwnership> {
        let mut taken = Vec::new();
        let mut remaining = Vec::new();
        for file_ownership in &self.files {
            if file_ownership.file_path == ownership.file_path {
                let (taken_ownership, remaining_ownership) = file_ownership.minus(ownership);
                if let Some(taken_ownership) = taken_ownership {
                    taken.push(taken_ownership);
                }
                if let Some(remaining_ownership) = remaining_ownership {
                    remaining.push(remaining_ownership);
                }
            } else {
                remaining.push(file_ownership.clone());
            }
        }

        self.files = remaining;

        taken
    }
}

#[cfg(test)]
mod tests {
    use std::vec;

    use super::*;

    #[test]
    fn test_ownership() {
        let ownership = Ownership::try_from("src/main.rs:0-100\nsrc/main.rs:200-300");
        assert!(ownership.is_ok());
        let ownership = ownership.unwrap();
        assert_eq!(ownership.files.len(), 1);
        assert_eq!(
            ownership.files[0],
            FileOwnership::try_from("src/main.rs:0-100,200-300").unwrap()
        );
    }

    #[test]
    fn test_ownership_2() {
        let ownership = Ownership::try_from("src/main.rs:0-100\nsrc/main2.rs:200-300");
        assert!(ownership.is_ok());
        let ownership = ownership.unwrap();
        assert_eq!(ownership.files.len(), 2);
        assert_eq!(
            ownership.files[0],
            FileOwnership::try_from("src/main.rs:0-100").unwrap()
        );
        assert_eq!(
            ownership.files[1],
            FileOwnership::try_from("src/main2.rs:200-300").unwrap()
        );
    }

    #[test]
    fn test_put() {
        let mut ownership = Ownership::try_from("src/main.rs:0-100").unwrap();
        ownership.put(&FileOwnership::try_from("src/main.rs:200-300").unwrap());
        assert_eq!(ownership.files.len(), 1);
        assert_eq!(
            ownership.files[0],
            FileOwnership::try_from("src/main.rs:0-100,200-300").unwrap()
        );
    }

    #[test]
    fn test_put_5() {
        let mut ownership = Ownership::try_from("src/main.rs:200-300").unwrap();
        ownership.put(&FileOwnership::try_from("src/main.rs:400-500").unwrap());
        assert_eq!(ownership.files.len(), 1);
        assert_eq!(
            ownership.files[0],
            FileOwnership::try_from("src/main.rs:200-300,400-500").unwrap()
        );
    }

    #[test]
    fn test_put_7() {
        let mut ownership = Ownership::try_from("src/main.rs:100-200").unwrap();
        ownership.put(&FileOwnership::try_from("src/main.rs:100-200").unwrap());
        assert_eq!(ownership.files.len(), 1);
        assert_eq!(
            ownership.files[0],
            FileOwnership::try_from("src/main.rs:100-200").unwrap()
        );
    }

    #[test]
    fn test_take_1() {
        let mut ownership = Ownership::try_from("src/main.rs:100-200,200-300").unwrap();
        let taken = ownership.take(&FileOwnership::try_from("src/main.rs:100-200").unwrap());
        assert_eq!(ownership.files.len(), 1);
        assert_eq!(
            ownership.files[0],
            FileOwnership::try_from("src/main.rs:200-300").unwrap()
        );
        assert_eq!(
            taken,
            vec![FileOwnership::try_from("src/main.rs:100-200").unwrap()]
        );
    }

    #[test]
    fn test_equal() {
        vec![
            (
                Ownership::try_from("src/main.rs:100-200").unwrap(),
                Ownership::try_from("src/main.rs:100-200").unwrap(),
                true,
            ),
            (
                Ownership::try_from("src/main.rs:100-200\nsrc/main1.rs:300-400\n").unwrap(),
                Ownership::try_from("src/main.rs:100-200").unwrap(),
                false,
            ),
            (
                Ownership::try_from("src/main.rs:100-200\nsrc/main1.rs:300-400\n").unwrap(),
                Ownership::try_from("src/main.rs:100-200\nsrc/main1.rs:300-400\n").unwrap(),
                true,
            ),
            (
                Ownership::try_from("src/main.rs:300-400\nsrc/main1.rs:100-200\n").unwrap(),
                Ownership::try_from("src/main1.rs:100-200\nsrc/main.rs:300-400\n").unwrap(),
                false,
            ),
        ]
        .into_iter()
        .for_each(|(a, b, expected)| {
            assert_eq!(a == b, expected, "{:#?} == {:#?}", a, b);
        });
    }
}
