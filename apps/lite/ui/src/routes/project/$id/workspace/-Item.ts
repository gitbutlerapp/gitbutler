import { getCommonBaseCommitId } from "#ui/domain/RefInfo.ts";
import { WorktreeChanges, type RefInfo } from "@gitbutler/but-sdk";
import { Match } from "effect";

export type ChangesSectionItem = { stackId: string | null };
export type ChangeItem = ChangesSectionItem & { path: string };

type SegmentItemBase = {
	stackId: string;
	segmentIndex: number;
	branchName: string | null;
};

type SegmentMode = { _tag: "Default" } | { _tag: "Rename" };

export type SegmentItem = SegmentItemBase & { mode: SegmentMode };

type CommitMode = { _tag: "Default" } | { _tag: "Details" } | { _tag: "Reword" };
export type CommitItem = SegmentItemBase & { commitId: string; mode: CommitMode };

export type BaseCommitItem = { commitId: string };

export type Item =
	| ({ _tag: "Changes" } & ChangesSectionItem)
	| ({ _tag: "Change" } & ChangeItem)
	| ({ _tag: "Segment" } & SegmentItem)
	| ({ _tag: "Commit" } & CommitItem)
	| ({ _tag: "BaseCommit" } & BaseCommitItem);

export const changesSectionItem = (stackId: string | null): Item => ({
	_tag: "Changes",
	stackId,
});

export const changeItem = (stackId: string | null, path: string): Item => ({
	_tag: "Change",
	stackId,
	path,
});

export const segmentItem = ({
	stackId,
	segmentIndex,
	branchName,
	mode = { _tag: "Default" },
}: Omit<SegmentItem, "mode"> & { mode?: SegmentItem["mode"] }): Item => ({
	_tag: "Segment",
	stackId,
	segmentIndex,
	branchName,
	mode,
});

export const commitItem = ({
	stackId,
	segmentIndex,
	branchName,
	commitId,
	mode = { _tag: "Default" },
}: Omit<CommitItem, "mode"> & { mode?: CommitItem["mode"] }): Item => ({
	_tag: "Commit",
	stackId,
	segmentIndex,
	branchName,
	commitId,
	mode,
});

export const baseCommitItem = (commitId: string): Item => ({
	_tag: "BaseCommit",
	commitId,
});

export const getParentSection = (item: Item): Item | null =>
	Match.value(item).pipe(
		Match.tagsExhaustive({
			Commit: (item): Item | null =>
				segmentItem({
					stackId: item.stackId,
					segmentIndex: item.segmentIndex,
					branchName: item.branchName,
				}),
			Change: (item): Item | null => changesSectionItem(item.stackId),
			Changes: () => null,
			BaseCommit: () => null,
			Segment: () => null,
		}),
	);

export const normalizeItem = (
	item: Item,
	headInfo: RefInfo,
	worktreeChanges: WorktreeChanges,
): Item | null =>
	Match.value(item).pipe(
		Match.tag("Changes", (item) => item),
		Match.tag("Change", (item) => {
			if (!worktreeChanges.changes.find((change) => change.path === item.path)) return null;
			if (
				!worktreeChanges.assignments.find(
					(assignment) => assignment.stackId === item.stackId && assignment.path === item.path,
				)
			)
				return null;
			return item;
		}),
		Match.tag("Segment", (item) => {
			const stack = headInfo.stacks.find((stack) => stack.id !== null && stack.id === item.stackId);
			if (!stack) return null;
			const segment = stack.segments[item.segmentIndex];
			if (!segment) return null;
			const branchName = segment.refName?.displayName ?? null;
			if (branchName !== item.branchName) return null;
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
