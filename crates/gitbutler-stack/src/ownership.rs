use std::{fmt, str::FromStr};

use anyhow::Result;
use but_meta::virtual_branches_legacy_types;

use crate::{Stack, file_ownership::OwnershipClaim};

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
