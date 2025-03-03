# Issue #7455: Local virtual branches get renamed when updating local workspace to origin

## Description

When updating the local workspace to origin (by clicking the "Update" button), unpushed branches in the local workspace get renamed (or shadowed) to `s-branch-X` format. For example:

- If a branch is named `fix/my_cool_bugfix`, it might become `s-branch-1`
- If a branch is already named `s-branch-1`, it might become `s-branch-2`

This issue affects the user experience as branch names are important for organization and context. When branches are automatically renamed, users lose the meaningful names they've assigned.

## Steps to Reproduce

1. Create a branch named `merge`, and a branch named `test`
2. Make some changes on the `merge` branch, and push them
3. Optionally, make some changes on the `test` branch, but do not push them
4. Create a PR for the `merge` branch
5. Merge the PR for `merge` on the remote
6. In GitButler, refresh the target, and click "Update"
7. Observe that the `test` branch is no longer named `test`, but instead `s-branch-X`

## Expected Behavior

The local branch names should not be affected when updating from origin. Branch names should be preserved during the update process.

## Actual Behavior

Local unpushed branches get renamed to `s-branch-X` format when updating the local workspace to origin.

## Additional Information

- If a user attempts to rename the branches back to their original names, GitButler prevents this with the error "That branch name already exists"
- This issue affects the workflow when working with multiple branches simultaneously, which is a core feature of GitButler

## Environment

- GitButler Version: 0.14.8
- Operating System: Windows
- Distribution Method: msi (Windows)

## Possible Solution

The update process should preserve the original branch names when integrating changes from the remote. If there are naming conflicts, the user should be prompted to resolve them rather than having branches automatically renamed.

## Technical Explanation

### Root Cause

The issue occurs in the `archive_integrated_heads` function in the branch management system. When a branch is fully integrated (all of its commits are now part of the target branch), GitButler archives the branch by moving it to the archive. However, when creating a new empty reference to represent the archived branch, the code was not preserving the original branch name.

Specifically:

1. When updating the local workspace to match the remote, GitButler identifies which local branches have been fully integrated into the target branch.
2. These branches are archived by moving their references to the archive.
3. For each archived branch, a new empty reference is created to maintain the branch's presence in the UI.
4. The issue was that this new empty reference was being created with a generated name (using the deduplication system) rather than preserving the original branch name.
5. This resulted in branches being renamed to the format `s-branch-X` instead of keeping their original names.

### Solution

The fix involves modifying the `archive_integrated_heads` function to preserve the original branch name when creating a new empty reference after archiving. Instead of generating a new name, we now use the original branch name for the new empty reference, ensuring that branch names remain consistent during updates.

This change ensures that when a branch is archived and a new empty reference is created, it maintains the same name as the original branch, preventing the unexpected renaming behavior.
