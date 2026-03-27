import { BranchIdentity } from "@gitbutler/but-sdk";
import { Match } from "effect";

type BranchMode = { _tag: "Summary" } | { _tag: "Details" };
export type BranchItem = { branchName: BranchIdentity; mode: BranchMode };

type CommitMode = { _tag: "Summary" } | { _tag: "Details"; path?: string };
export type CommitItem = { branchName: BranchIdentity; commitId: string; mode: CommitMode };

export type Item = ({ _tag: "Branch" } & BranchItem) | ({ _tag: "Commit" } & CommitItem);

export const branchSummaryItem = (branchName: BranchIdentity): Item => ({
	_tag: "Branch",
	branchName,
	mode: { _tag: "Summary" },
});

export const branchDetailsItem = (branchName: BranchIdentity): Item => ({
	_tag: "Branch",
	branchName,
	mode: { _tag: "Details" },
});

export const commitSummaryItem = (branchName: BranchIdentity, commitId: string): Item => ({
	_tag: "Commit",
	branchName,
	commitId,
	mode: { _tag: "Summary" },
});

export const commitDetailsItem = (
	branchName: BranchIdentity,
	commitId: string,
	path?: string,
): Item => ({
	_tag: "Commit",
	branchName,
	commitId,
	mode: { _tag: "Details", path },
});

export const getParentItem = (item: Item): Item | null =>
	Match.value(item).pipe(
		Match.tag("Commit", (item): Item =>
			item.mode._tag === "Details"
				? commitSummaryItem(item.branchName, item.commitId)
				: branchDetailsItem(item.branchName),
		),
		Match.tag("Branch", (item): Item | null =>
			item.mode._tag === "Details" ? branchSummaryItem(item.branchName) : null,
		),
		Match.exhaustive,
	);
