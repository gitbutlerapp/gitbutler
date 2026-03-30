import { getCommonBaseCommitId, getSegmentBranchRef } from "#ui/domain/RefInfo.ts";
import { WorktreeChanges, type RefInfo } from "@gitbutler/but-sdk";
import { Match } from "effect";

export type ChangesMode = { _tag: "Summary" } | { _tag: "Details"; path?: string };
export type ChangesItem = { stackId: string | null; mode: ChangesMode };

export type SegmentItem = {
	stackId: string;
	segmentIndex: number;
	branchName: string | null;
	branchRef: string | null;
};

type CommitMode = { _tag: "Summary" } | { _tag: "Details"; path?: string };
export type CommitItem = SegmentItem & { commitId: string; mode: CommitMode };

export type BaseCommitItem = { commitId: string };

export type Item =
	| ({ _tag: "Changes" } & ChangesItem)
	| ({ _tag: "Segment" } & SegmentItem)
	| ({ _tag: "Commit" } & CommitItem)
	| ({ _tag: "BaseCommit" } & BaseCommitItem);

export const changesSummaryItem = (stackId: string | null): Item => ({
	_tag: "Changes",
	stackId,
	mode: { _tag: "Summary" },
});

export const changesDetailsItem = (stackId: string | null, path?: string): Item => ({
	_tag: "Changes",
	stackId,
	mode: { _tag: "Details", path },
});

export const segmentItem = ({
	stackId,
	segmentIndex,
	branchName,
	branchRef,
}: SegmentItem): Item => ({
	_tag: "Segment",
	stackId,
	segmentIndex,
	branchName,
	branchRef,
});

export const commitItem = ({
	stackId,
	segmentIndex,
	branchName,
	branchRef,
	commitId,
	mode = { _tag: "Summary" },
}: Omit<CommitItem, "mode"> & { mode?: CommitItem["mode"] }): Item => ({
	_tag: "Commit",
	stackId,
	segmentIndex,
	branchName,
	branchRef,
	commitId,
	mode,
});

export const baseCommitItem = (commitId: string): Item => ({
	_tag: "BaseCommit",
	commitId,
});

export const getParentSection = (selection: Item): Item | null =>
	Match.value(selection).pipe(
		Match.tag("Commit", (item): Item | null => segmentItem(item)),
		Match.tag("Changes", (item): Item | null =>
			item.mode._tag === "Details" ? changesSummaryItem(item.stackId) : null,
		),
		Match.tag("BaseCommit", () => null),
		Match.tag("Segment", () => null),
		Match.exhaustive,
	);

export const itemKey = (item: Item): string =>
	Match.value(item).pipe(
		Match.tag("Changes", (item) =>
			item.mode._tag === "Details"
				? JSON.stringify(["Changes", item.stackId, "Details", item.mode.path ?? null])
				: JSON.stringify(["Changes", item.stackId, item.mode._tag]),
		),
		Match.tag("Segment", (item) => JSON.stringify(["Segment", item.stackId, item.segmentIndex])),
		Match.tag("Commit", (item) =>
			JSON.stringify(["Commit", item.stackId, item.segmentIndex, item.commitId]),
		),
		Match.tag("BaseCommit", (item) => JSON.stringify(["BaseCommit", item.commitId])),
		Match.exhaustive,
	);

export const normalizeItem = (
	item: Item,
	headInfo: RefInfo,
	worktreeChanges: WorktreeChanges,
): Item | null =>
	Match.value(item).pipe(
		Match.tag("Changes", (item) =>
			Match.value(item.mode).pipe(
				Match.tag("Summary", () => item),
				Match.tag("Details", (mode) => {
					if (mode.path === undefined) return item;

					if (!worktreeChanges.changes.find((change) => change.path === mode.path)) return null;
					if (
						!worktreeChanges.assignments.find(
							(assignment) => assignment.stackId === item.stackId && assignment.path === mode.path,
						)
					)
						return null;
					return item;
				}),
				Match.exhaustive,
			),
		),
		Match.tag("Segment", (item) => {
			const stack = headInfo.stacks.find((stack) => stack.id !== null && stack.id === item.stackId);
			if (!stack) return null;
			const segment = stack.segments[item.segmentIndex];
			if (!segment) return null;
			const branchName = segment.refName?.displayName ?? null;
			const branchRef = segment.refName ? getSegmentBranchRef(segment.refName) : null;
			if (branchName !== item.branchName || branchRef !== item.branchRef) return null;
			return item;
		}),
		Match.tag("Commit", (item) => {
			const stack = headInfo.stacks.find((stack) => stack.id !== null && stack.id === item.stackId);
			if (!stack) return null;
			const segment = stack.segments[item.segmentIndex];
			if (!segment) return null;
			if (!segment.commits.some((commit) => commit.id === item.commitId)) return null;
			return item;
		}),
		Match.tag("BaseCommit", (item) => {
			const commonBaseCommitId = getCommonBaseCommitId(headInfo);
			return commonBaseCommitId === item.commitId ? item : null;
		}),
		Match.exhaustive,
	);
