use std::{collections::HashSet, fmt, str::FromStr};

use anyhow::Result;
use itertools::Itertools;
use serde::{Deserialize, Serialize, Serializer};

use super::{Branch, OwnershipClaim};

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

    pub fn put(&mut self, ownership: OwnershipClaim) {
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
            self.claims.insert(0, ownership);
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
    new_claims: &[OwnershipClaim],
) -> Result<Vec<ClaimOutcome>> {
    let mut other_branches = all_branches
        .into_iter()
        .filter(|branch| branch.id != claiming_branch.id)
        .collect::<Vec<_>>();

    let mut claim_outcomes: Vec<ClaimOutcome> = Vec::new();

    for branch in &mut other_branches {
        let taken = new_claims
            .iter()
            .flat_map(|c| branch.ownership.take(c))
            .collect_vec();
        claim_outcomes.push(ClaimOutcome {
            updated_branch: branch.clone(),
            removed_claims: taken,
        });
    }

    // Add the claiming branch to the list of outcomes
    claim_outcomes.push(ClaimOutcome {
        updated_branch: Branch {
            ownership: BranchOwnershipClaims {
                claims: new_claims.to_owned(),
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
