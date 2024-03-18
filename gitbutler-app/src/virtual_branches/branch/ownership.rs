use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize, Serializer};

use super::OwnershipClaim;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BranchOwnershipClaims {
    pub claims: Vec<OwnershipClaim>,
}

impl Serialize for BranchOwnershipClaims {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.to_string().as_str())
    }
}

impl<'de> Deserialize<'de> for BranchOwnershipClaims {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for BranchOwnershipClaims {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for file in &self.claims {
            writeln!(f, "{}", file)?;
        }
        Ok(())
    }
}

impl FromStr for BranchOwnershipClaims {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut ownership = BranchOwnershipClaims::default();
        for line in s.lines() {
            ownership.claims.push(line.parse()?);
        }
        Ok(ownership)
    }
}

impl BranchOwnershipClaims {
    pub fn is_empty(&self) -> bool {
        self.claims.is_empty()
    }

    pub fn contains(&self, another: &BranchOwnershipClaims) -> bool {
        if another.is_empty() {
            return true;
        }

        if self.is_empty() {
            return false;
        }

        for file_ownership in &another.claims {
            let mut found = false;
            for self_file_ownership in &self.claims {
                if self_file_ownership.file_path == file_ownership.file_path
                    && self_file_ownership.contains(file_ownership)
                {
                    found = true;
                    break;
                }
            }
            if !found {
                return false;
            }
        }

        true
    }

    pub fn put(&mut self, ownership: &OwnershipClaim) {
        let target = self
            .claims
            .iter()
            .filter(|o| !o.is_full()) // only consider explicit ownership
            .find(|o| o.file_path == ownership.file_path)
            .cloned();

        self.claims
            .retain(|o| o.is_full() || o.file_path != ownership.file_path);

        if let Some(target) = target {
            self.claims.insert(0, target.plus(ownership));
        } else {
            self.claims.insert(0, ownership.clone());
        }
    }

    // modifies the ownership in-place and returns the file ownership that was taken, if any.
    pub fn take(&mut self, ownership: &OwnershipClaim) -> Vec<OwnershipClaim> {
        let mut taken = Vec::new();
        let mut remaining = Vec::new();
        for file_ownership in &self.claims {
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

        self.claims = remaining;

        taken
    }
}

#[cfg(test)]
mod tests {
    use std::vec;

    use super::*;

    #[test]
    fn test_ownership() {
        let ownership = "src/main.rs:0-100\nsrc/main2.rs:200-300".parse::<BranchOwnershipClaims>();
        assert!(ownership.is_ok());
        let ownership = ownership.unwrap();
        assert_eq!(ownership.claims.len(), 2);
        assert_eq!(
            ownership.claims[0],
            "src/main.rs:0-100".parse::<OwnershipClaim>().unwrap()
        );
        assert_eq!(
            ownership.claims[1],
            "src/main2.rs:200-300".parse::<OwnershipClaim>().unwrap()
        );
    }

    #[test]
    fn test_ownership_2() {
        let ownership = "src/main.rs:0-100\nsrc/main2.rs:200-300".parse::<BranchOwnershipClaims>();
        assert!(ownership.is_ok());
        let ownership = ownership.unwrap();
        assert_eq!(ownership.claims.len(), 2);
        assert_eq!(
            ownership.claims[0],
            "src/main.rs:0-100".parse::<OwnershipClaim>().unwrap()
        );
        assert_eq!(
            ownership.claims[1],
            "src/main2.rs:200-300".parse::<OwnershipClaim>().unwrap()
        );
    }

    #[test]
    fn test_put() {
        let mut ownership = "src/main.rs:0-100"
            .parse::<BranchOwnershipClaims>()
            .unwrap();
        ownership.put(&"src/main.rs:200-300".parse::<OwnershipClaim>().unwrap());
        assert_eq!(ownership.claims.len(), 1);
        assert_eq!(
            ownership.claims[0],
            "src/main.rs:200-300,0-100"
                .parse::<OwnershipClaim>()
                .unwrap()
        );
    }

    #[test]
    fn test_put_2() {
        let mut ownership = "src/main.rs:0-100"
            .parse::<BranchOwnershipClaims>()
            .unwrap();
        ownership.put(&"src/main.rs2:200-300".parse::<OwnershipClaim>().unwrap());
        assert_eq!(ownership.claims.len(), 2);
        assert_eq!(
            ownership.claims[0],
            "src/main.rs2:200-300".parse::<OwnershipClaim>().unwrap()
        );
        assert_eq!(
            ownership.claims[1],
            "src/main.rs:0-100".parse::<OwnershipClaim>().unwrap()
        );
    }

    #[test]
    fn test_put_3() {
        let mut ownership = "src/main.rs:0-100\nsrc/main2.rs:100-200"
            .parse::<BranchOwnershipClaims>()
            .unwrap();
        ownership.put(&"src/main2.rs:200-300".parse::<OwnershipClaim>().unwrap());
        assert_eq!(ownership.claims.len(), 2);
        assert_eq!(
            ownership.claims[0],
            "src/main2.rs:200-300,100-200"
                .parse::<OwnershipClaim>()
                .unwrap()
        );
        assert_eq!(
            ownership.claims[1],
            "src/main.rs:0-100".parse::<OwnershipClaim>().unwrap()
        );
    }

    #[test]
    fn test_put_4() {
        let mut ownership = "src/main.rs:0-100\nsrc/main2.rs:100-200"
            .parse::<BranchOwnershipClaims>()
            .unwrap();
        ownership.put(&"src/main2.rs:100-200".parse::<OwnershipClaim>().unwrap());
        assert_eq!(ownership.claims.len(), 2);
        assert_eq!(
            ownership.claims[0],
            "src/main2.rs:100-200".parse::<OwnershipClaim>().unwrap()
        );
        assert_eq!(
            ownership.claims[1],
            "src/main.rs:0-100".parse::<OwnershipClaim>().unwrap()
        );
    }

    #[test]
    fn test_put_7() {
        let mut ownership = "src/main.rs:100-200"
            .parse::<BranchOwnershipClaims>()
            .unwrap();
        ownership.put(&"src/main.rs:100-200".parse::<OwnershipClaim>().unwrap());
        assert_eq!(ownership.claims.len(), 1);
        assert_eq!(
            ownership.claims[0],
            "src/main.rs:100-200".parse::<OwnershipClaim>().unwrap()
        );
    }

    #[test]
    fn test_take_1() {
        let mut ownership = "src/main.rs:100-200,200-300"
            .parse::<BranchOwnershipClaims>()
            .unwrap();
        let taken = ownership.take(&"src/main.rs:100-200".parse::<OwnershipClaim>().unwrap());
        assert_eq!(ownership.claims.len(), 1);
        assert_eq!(
            ownership.claims[0],
            "src/main.rs:200-300".parse::<OwnershipClaim>().unwrap()
        );
        assert_eq!(
            taken,
            vec!["src/main.rs:100-200".parse::<OwnershipClaim>().unwrap()]
        );
    }

    #[test]
    fn test_equal() {
        for (a, b, expected) in vec![
            (
                "src/main.rs:100-200"
                    .parse::<BranchOwnershipClaims>()
                    .unwrap(),
                "src/main.rs:100-200"
                    .parse::<BranchOwnershipClaims>()
                    .unwrap(),
                true,
            ),
            (
                "src/main.rs:100-200\nsrc/main1.rs:300-400\n"
                    .parse::<BranchOwnershipClaims>()
                    .unwrap(),
                "src/main.rs:100-200"
                    .parse::<BranchOwnershipClaims>()
                    .unwrap(),
                false,
            ),
            (
                "src/main.rs:100-200\nsrc/main1.rs:300-400\n"
                    .parse::<BranchOwnershipClaims>()
                    .unwrap(),
                "src/main.rs:100-200\nsrc/main1.rs:300-400\n"
                    .parse::<BranchOwnershipClaims>()
                    .unwrap(),
                true,
            ),
            (
                "src/main.rs:300-400\nsrc/main1.rs:100-200\n"
                    .parse::<BranchOwnershipClaims>()
                    .unwrap(),
                "src/main1.rs:100-200\nsrc/main.rs:300-400\n"
                    .parse::<BranchOwnershipClaims>()
                    .unwrap(),
                false,
            ),
        ] {
            assert_eq!(a == b, expected, "{:#?} == {:#?}", a, b);
        }
    }
}
