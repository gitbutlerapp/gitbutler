import { splitMessage } from "$lib/commits/commitMessage";
import type { RemoteCommit } from "@gitbutler/but-sdk";

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
