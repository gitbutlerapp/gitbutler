import type { BranchIconName } from "$lib/branches/branchIcon";
import type { CommitStatusType } from "$lib/commits/commit";
import type { Commit, CommitState, PushStatus, UpstreamCommit } from "@gitbutler/but-sdk";

const colorMap: Record<CommitStatusType, string> & { Error: string } = {
	LocalOnly: "var(--commit-local)",
	LocalAndRemote: "var(--commit-remote)",
	Integrated: "var(--commit-integrated)",
	Remote: "var(--commit-upstream)", // TODO: rename Remote -> Upstream.
	Base: "var(--commit-upstream)", // TODO: Introduce separate color for base.
	Error: "var(--fill-danger-bg)",
};

export function getIconFromCommitState(
	commitId?: string,
	commitState?: CommitState,
): BranchIconName {
	if (!commitId || !commitState) {
		return "branch-local";
	}
	switch (commitState.type) {
		case "LocalOnly":
			return "branch-local";
		case "LocalAndRemote":
			return commitState.subject !== commitId ? "branch-double-commit" : "branch";
		case "Integrated":
			return "branch-merge";
		default:
			return "branch-local";
	}
}

export function getColorFromCommitState(
	commitType: CommitStatusType,
	diverged: boolean = false,
): string {
	if (diverged) {
		return colorMap.LocalOnly;
	}

	return colorMap[commitType];
}

export function isUpstreamCommit(commit: Commit | UpstreamCommit): commit is UpstreamCommit {
	return !("state" in commit);
}

export function isLocalAndRemoteCommit(commit: Commit | UpstreamCommit): commit is Commit {
	return "state" in commit;
}

export function hasConflicts(commit: Commit): boolean {
	return commit.hasConflicts;
}

export function getBranchStatusLabelAndColor(pushStatus: PushStatus): {
	label: string;
	color: string;
} {
	switch (pushStatus) {
		case "completelyUnpushed":
			return { label: "Unpushed branch", color: colorMap.LocalOnly };
		case "nothingToPush":
			return { label: "Nothing to push", color: colorMap.LocalAndRemote };
		case "unpushedCommits":
		case "unpushedCommitsRequiringForce":
			return { label: "Some unpushed", color: colorMap.LocalAndRemote };
		case "integrated":
			return { label: "Integrated", color: colorMap.Integrated };
		default:
			return { label: "Unknown", color: colorMap.Error };
	}
}
