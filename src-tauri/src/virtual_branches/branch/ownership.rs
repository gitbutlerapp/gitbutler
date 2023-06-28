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

        self.files.sort_by(|a, b| a.file_path.cmp(&b.file_path));
        self.files.dedup();
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
        self.files.sort_by(|a, b| a.file_path.cmp(&b.file_path));
        self.files.dedup();

        taken
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
    fn test_put_2() {
        let mut ownership = Ownership::try_from("src/main.rs").unwrap();
        ownership.put(&FileOwnership::try_from("src/main.rs:200-300").unwrap());
        assert_eq!(ownership.files.len(), 2);
        assert_eq!(
            ownership.files[0],
            FileOwnership::try_from("src/main.rs").unwrap()
        );
        assert_eq!(
            ownership.files[1],
            FileOwnership::try_from("src/main.rs:200-300").unwrap()
        );
    }

    #[test]
    fn test_put_4() {
        let mut ownership = Ownership::try_from("src/main.rs:200-300").unwrap();
        ownership.put(&FileOwnership::try_from("src/main.rs").unwrap());
        assert_eq!(ownership.files.len(), 2);
        assert_eq!(
            ownership.files[0],
            FileOwnership::try_from("src/main.rs:200-300").unwrap()
        );
        assert_eq!(
            ownership.files[1],
            FileOwnership::try_from("src/main.rs").unwrap()
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
    fn test_put_6() {
        let mut ownership = Ownership::try_from("src/main.rs").unwrap();
        ownership.put(&FileOwnership::try_from("src/main.rs").unwrap());
        assert_eq!(ownership.files.len(), 1);
        assert_eq!(
            ownership.files[0],
            FileOwnership::try_from("src/main.rs").unwrap()
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
    fn test_take() {
        let mut ownership = Ownership::try_from("src/main.rs").unwrap();
        let taken = ownership.take(&FileOwnership::try_from("src/main.rs").unwrap());
        assert_eq!(ownership.files.len(), 0);
        assert_eq!(taken, vec![FileOwnership::try_from("src/main.rs").unwrap()]);
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
    fn test_take_2() {
        let mut ownership =
            Ownership::try_from("src/main.rs:100-200,200-300\nsrc/main.rs").unwrap();
        let taken = ownership.take(&FileOwnership::try_from("src/main.rs:100-200").unwrap());
        println!("{}", ownership);
        println!("{:?}", taken);
        assert_eq!(ownership.files.len(), 2);
        assert_eq!(
            ownership.files[0],
            FileOwnership::try_from("src/main.rs:200-300").unwrap()
        );
        assert_eq!(
            ownership.files[1],
            FileOwnership::try_from("src/main.rs").unwrap()
        );
        assert_eq!(
            taken,
            vec![FileOwnership::try_from("src/main.rs:100-200").unwrap()]
        );
    }
}
