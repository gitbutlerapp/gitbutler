import type { IconName } from "#ui/components/iconNames.ts";
import type { SnapshotDetails } from "@gitbutler/but-sdk";

export const presentableOperation = (
	snapshotDetails: SnapshotDetails | null,
): { text: string; icon: IconName } => {
	switch (snapshotDetails?.operation) {
		case "Absorb":
			return { text: "Absorb changes into commit", icon: "absorb" };
		case "AmendCommit":
			return { text: "Amend commit", icon: "edit" };
		case "ApplyBranch":
			return { text: "Apply branch", icon: "branch" };
		case "AutoCommit":
			return { text: "Auto commit changes", icon: "ai" };
		case "AutoHandleChangesAfter":
			return { text: "Handle changes after action", icon: "refresh" };
		case "AutoHandleChangesBefore":
			return { text: "Handle changes before action", icon: "refresh" };
		case "CherryPick":
			return { text: "Cherry-pick commit", icon: "commit" };
		case "CleanWorkspace":
			return { text: "Clean workspace", icon: "cross" };
		case "CreateBranch":
			return { text: "Create branch", icon: "plus" };
		case "CreateCommit":
			return { text: "Create commit", icon: "plus" };
		case "CreateDependentBranch":
			return { text: "Create branch", icon: "plus" };
		case "DeleteBranch":
			return { text: "Delete branch", icon: "cross" };
		case "Discard":
			return { text: "Discard changes", icon: "cross" };
		case "DiscardChanges":
			return { text: "Discard changes", icon: "cross" };
		case "DiscardCommit":
			return { text: "Discard commit", icon: "cross" };
		case "DiscardFile":
			return { text: "Discard file", icon: "cross" };
		case "DiscardHunk":
			return { text: "Discard hunk", icon: "cross" };
		case "DiscardLines":
			return { text: "Discard lines", icon: "cross" };
		case "EnterEditMode":
			return { text: "Enter Edit Mode", icon: "edit" };
		case "FileChanges":
			return { text: "File changes", icon: "file" };
		case "GenericBranchUpdate":
			return { text: "Generic branch update", icon: "branch" };
		case "InsertBlankCommit":
			return { text: "Insert blank commit", icon: "plus" };
		case "MergeUpstream":
			return { text: "Merge upstream", icon: "pr" };
		case "MoveBranch":
			return { text: "Move branch", icon: "branch" };
		case "MoveCommit":
			return { text: "Move commit", icon: "commit" };
		case "MoveCommitFile":
			return { text: "Move commit file", icon: "commit" };
		case "MoveHunk":
			return { text: "Move hunk", icon: "file" };
		case "OnDemandSnapshot":
			return {
				text:
					snapshotDetails.body !== null && snapshotDetails.body !== ""
						? `Manual snapshot: ${snapshotDetails.body}`
						: "Manual snapshot",
				icon: "commit",
			};
		case "RemoveDependentBranch":
			return { text: "Remove branch", icon: "branch" };
		case "ReorderBranches":
			return { text: "Reorder branches", icon: "branch" };
		case "ReorderCommit":
			return { text: "Reorder commit", icon: "commit" };
		case "ResolveConflicts":
			return { text: "Resolve conflicts", icon: "tick" };
		case "ResolveConflictsAi":
			return { text: "Resolve conflicts with AI", icon: "ai" };
		case "RestoreFromSnapshot":
			return { text: "Revert snapshot", icon: "undo" };
		case "RestoreFromSnapshotViaRedo":
			return { text: "Revert snapshot", icon: "undo" };
		case "RestoreFromSnapshotViaUndo":
			return { text: "Revert snapshot", icon: "undo" };
		case "SetBaseBranch":
			return { text: "Set base branch", icon: "branch" };
		case "SplitBranch":
			return { text: "Split branch", icon: "branch" };
		case "SquashCommit":
			return { text: "Squash commit", icon: "commit" };
		case "StashIntoBranch":
			return { text: "Stash into branch", icon: "branch" };
		case "SyncWorkspace":
			return { text: "Sync workspace", icon: "refresh" };
		case "TearOffBranch":
			return { text: "Tear off branch", icon: "branch" };
		case "UnapplyBranch":
			return { text: "Unapply branch", icon: "branch" };
		case "UndoCommit":
			return { text: "Undo commit", icon: "undo" };
		case "Unknown":
			return { text: "Unknown operation", icon: "commit" };
		case "UpdateBranchName":
			return { text: "Rename branch", icon: "edit" };
		case "UpdateBranchNotes":
			return { text: "Update branch notes", icon: "edit" };
		case "UpdateBranchRemoteName":
			return { text: "Update branch remote name", icon: "edit" };
		case "UpdateCommitMessage":
			return { text: "Update commit message", icon: "edit" };
		case "UpdateDependentBranchDescription":
			return { text: "Update branch description", icon: "edit" };
		case "UpdateDependentBranchName":
			return { text: "Update branch name", icon: "edit" };
		case "UpdateDependentBranchPrNumber":
			return { text: "Update branch pull request number", icon: "edit" };
		case "UpdateWorkspaceBase":
			return { text: "Update workspace base", icon: "refresh" };
		case undefined:
			return { text: "Unknown operation", icon: "question" };
	}
};
