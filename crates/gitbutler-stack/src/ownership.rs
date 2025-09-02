use std::{collections::HashSet, fmt, str::FromStr};

use crate::{file_ownership::OwnershipClaim, Stack};
use anyhow::Result;
use but_graph::virtual_branches_legacy_types;
use itertools::Itertools;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BranchOwnershipClaims {
    pub claims: Vec<OwnershipClaim>,
}

impl From<virtual_branches_legacy_types::BranchOwnershipClaims> for BranchOwnershipClaims {
    fn from(
        virtual_branches_legacy_types::BranchOwnershipClaims{ claims }: virtual_branches_legacy_types::BranchOwnershipClaims,
    ) -> Self {
        BranchOwnershipClaims {
            claims: claims.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<BranchOwnershipClaims> for virtual_branches_legacy_types::BranchOwnershipClaims {
    fn from(BranchOwnershipClaims { claims }: BranchOwnershipClaims) -> Self {
        virtual_branches_legacy_types::BranchOwnershipClaims {
            claims: claims.into_iter().map(Into::into).collect(),
        }
    }
}

impl fmt::Display for BranchOwnershipClaims {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for file in &self.claims {
            writeln!(f, "{file}")?;
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
    pub updated_branch: Stack,
    pub removed_claims: Vec<OwnershipClaim>,
}
pub fn reconcile_claims(
    all_branches: Vec<Stack>,
    claiming_stack: &Stack,
    new_claims: &[OwnershipClaim],
) -> Result<Vec<ClaimOutcome>> {
    let mut other_branches = all_branches
        .into_iter()
        .filter(|branch| branch.id != claiming_stack.id)
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

    let mut updated_branch = claiming_stack.clone();
    updated_branch.ownership.claims = new_claims.to_owned();

    // Add the claiming branch to the list of outcomes
    claim_outcomes.push(ClaimOutcome {
        updated_branch,
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
