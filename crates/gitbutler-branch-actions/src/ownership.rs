use std::path::PathBuf;

use anyhow::{anyhow, Result};
use gitbutler_diff::{DiffByPathMap, GitHunk};
use gitbutler_stack::BranchOwnershipClaims;
use itertools::Itertools;

pub fn filter_hunks_by_ownership(
    diffs: &DiffByPathMap,
    ownership: &BranchOwnershipClaims,
) -> Result<Vec<(PathBuf, Vec<GitHunk>)>> {
    ownership
        .claims
        .iter()
        .map(|claim| {
            if let Some(diff) = diffs.get(&claim.file_path) {
                let hunks = claim
                    .hunks
                    .iter()
                    .filter_map(|claimed_hunk| {
                        diff.hunks
                            .iter()
                            .find(|diff_hunk| {
                                claimed_hunk.start == diff_hunk.new_start
                                    && claimed_hunk.end == diff_hunk.new_start + diff_hunk.new_lines
                            })
                            .cloned()
                    })
                    .collect_vec();
                Ok((claim.file_path.clone(), hunks))
            } else {
                Err(anyhow!("Claim not found in workspace diff"))
            }
        })
        .collect::<Result<Vec<(_, Vec<_>)>>>()
}
