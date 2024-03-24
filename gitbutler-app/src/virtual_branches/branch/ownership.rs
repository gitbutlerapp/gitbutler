use std::{collections::HashSet, fmt, str::FromStr};

use serde::{Deserialize, Serialize, Serializer};

use super::{Branch, OwnershipClaim};
use anyhow::Result;

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

#[derive(Debug, Clone)]
pub struct ClaimOutcome {
    pub updated_branch: Branch,
    pub removed_claims: Vec<OwnershipClaim>,
}
pub fn reconcile_claims(
    all_branches: Vec<Branch>,
    claiming_branch: &Branch,
    new_claims: &Vec<OwnershipClaim>,
) -> Result<Vec<ClaimOutcome>> {
    let mut other_branches = all_branches
        .into_iter()
        .filter(|branch| branch.applied)
        .filter(|branch| branch.id != claiming_branch.id)
        .collect::<Vec<_>>();

    let mut claim_outcomes: Vec<ClaimOutcome> = Vec::new();

    for file_ownership in new_claims {
        for branch in &mut other_branches {
            let taken = branch.ownership.take(file_ownership);
            claim_outcomes.push(ClaimOutcome {
                updated_branch: branch.clone(),
                removed_claims: taken,
            });
        }
    }

    // Add the claiming branch to the list of outcomes
    claim_outcomes.push(ClaimOutcome {
        updated_branch: Branch {
            ownership: BranchOwnershipClaims {
                claims: new_claims.clone(),
            },
            ..claiming_branch.clone()
        },
        removed_claims: Vec::new(),
    });

    // Check the outcomes consistency and error out if they would result in a hunk being claimed by multiple branches
    let mut seen = HashSet::new();
    for outcome in claim_outcomes.clone() {
        for claim in outcome.updated_branch.ownership.claims {
            for hunk in claim.hunks {
                if !seen.insert(format!(
                    "{}-{}-{}",
                    claim.file_path.to_str().unwrap_or_default(),
                    hunk.start,
                    hunk.end
                )) {
                    return Err(anyhow::anyhow!("inconsistent ownership claims"));
                }
            }
        }
    }

    Ok(claim_outcomes)
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, vec};

    use crate::virtual_branches::branch::Hunk;

    use super::*;

    #[test]
    fn test_reconcile_ownership_simple() {
        let branch_a = Branch {
            name: "a".to_string(),
            ownership: BranchOwnershipClaims {
                claims: vec![OwnershipClaim {
                    file_path: PathBuf::from("foo"),
                    hunks: vec![
                        Hunk {
                            start: 1,
                            end: 3,
                            hash: Some("1,3".to_string()),
                            timestamp_ms: None,
                        },
                        Hunk {
                            start: 4,
                            end: 6,
                            hash: Some("4,6".to_string()),
                            timestamp_ms: None,
                        },
                    ],
                }],
            },
            applied: true,
            ..Default::default()
        };
        let branch_b = Branch {
            name: "b".to_string(),
            ownership: BranchOwnershipClaims {
                claims: vec![OwnershipClaim {
                    file_path: PathBuf::from("foo"),
                    hunks: vec![Hunk {
                        start: 7,
                        end: 9,
                        hash: Some("7,9".to_string()),
                        timestamp_ms: None,
                    }],
                }],
            },
            applied: true,
            ..Default::default()
        };
        let all_branches: Vec<Branch> = vec![branch_a.clone(), branch_b.clone()];
        let claim: Vec<OwnershipClaim> = vec![OwnershipClaim {
            file_path: PathBuf::from("foo"),
            hunks: vec![
                Hunk {
                    start: 4,
                    end: 6,
                    hash: Some("4,6".to_string()),
                    timestamp_ms: None,
                },
                Hunk {
                    start: 7,
                    end: 9,
                    hash: Some("9,7".to_string()),
                    timestamp_ms: None,
                },
            ],
        }];
        let claim_outcomes = reconcile_claims(all_branches.clone(), &branch_b, &claim).unwrap();
        assert_eq!(claim_outcomes.len(), all_branches.len());
        assert_eq!(claim_outcomes[0].updated_branch.id, branch_a.id);
        assert_eq!(claim_outcomes[1].updated_branch.id, branch_b.id);

        assert_eq!(
            claim_outcomes[0].updated_branch.ownership,
            BranchOwnershipClaims {
                claims: vec![OwnershipClaim {
                    file_path: PathBuf::from("foo"),
                    hunks: vec![Hunk {
                        start: 1,
                        end: 3,
                        hash: Some("1,3".to_string()),
                        timestamp_ms: None,
                    },],
                }],
            }
        );

        assert_eq!(
            claim_outcomes[1].updated_branch.ownership,
            BranchOwnershipClaims {
                claims: vec![OwnershipClaim {
                    file_path: PathBuf::from("foo"),
                    hunks: vec![
                        Hunk {
                            start: 4,
                            end: 6,
                            hash: Some("4,6".to_string()),
                            timestamp_ms: None,
                        },
                        Hunk {
                            start: 7,
                            end: 9,
                            hash: Some("9,7".to_string()),
                            timestamp_ms: None,
                        },
                    ],
                }],
            }
        );
    }

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
