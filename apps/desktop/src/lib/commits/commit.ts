import { splitMessage } from "$lib/commits/commitMessage";
import type { RemoteCommit } from "@gitbutler/but-sdk";

export type CommitKey = {
	stackId?: string;
	branchName: string;
	commitId: string;
	upstream: boolean;
};

export function descriptionTitle(commit: { description: string }): string | undefined {
	return splitMessage(commit.description).title || undefined;
}

export function descriptionBody(commit: { description: string }): string | undefined {
	return splitMessage(commit.description).description || undefined;
}

export function isParentOf(parent: RemoteCommit, possibleChild: RemoteCommit): boolean {
	return possibleChild.parentIds.includes(parent.id);
}

export function isMergeCommit(commit: { parentIds: string[] }): boolean {
	return commit.parentIds.length > 1;
}

export enum CommitStatus {
	LocalOnly = "LocalOnly",
	LocalAndRemote = "LocalAndRemote",
	Integrated = "Integrated",
	Remote = "Remote",
	Base = "Base",
}

export type CommitStatusType = keyof typeof CommitStatus;

export function commitStatusLabel(status: CommitStatusType): string {
	switch (status) {
		case CommitStatus.LocalOnly:
			return "Local";
		case CommitStatus.LocalAndRemote:
			return "Local and remote";
		case CommitStatus.Integrated:
			return "Integrated";
		case CommitStatus.Remote:
			return "Remote";
		case CommitStatus.Base:
			return "Base";
		default:
			return status;
	}
}

export type MoveCommitIllegalAction =
	| {
			type: "dependsOnCommits";
			subject: string[];
	  }
	| {
			type: "hasDependentChanges";
			subject: string[];
	  }
	| {
			type: "hasDependentUncommittedChanges";
	  };

function formatCommitIds(ids: string[]): string {
	return ids.map((id) => id.slice(0, 7)).join("\n");
}

export function getMoveCommitIllegalActionMessage(action: MoveCommitIllegalAction): string {
	switch (action.type) {
		case "dependsOnCommits":
			return `Cannot move commit because it depends on the following commits: ${formatCommitIds(action.subject)}`;
		case "hasDependentChanges":
			return `Cannot move commit because it has dependent changes: ${formatCommitIds(action.subject)}`;
		case "hasDependentUncommittedChanges":
			return `Cannot move commit because it has dependent uncommitted changes`;
	}
}
