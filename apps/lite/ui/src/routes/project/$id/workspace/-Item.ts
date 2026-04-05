import { Match } from "effect";

export type ChangesSectionItem = { stackId: string | null };
export type ChangeItem = ChangesSectionItem & { path: string };

type SegmentItemBase = {
	stackId: string;
	segmentIndex: number;
	branchName: string | null;
};

export type SegmentMode = { _tag: "Default" } | { _tag: "Rename" };
export const defaultSegmentMode: SegmentMode = { _tag: "Default" };

export type SegmentItem = SegmentItemBase & { mode: SegmentMode };

export type CommitMode = { _tag: "Default" } | { _tag: "Details" } | { _tag: "Reword" };
export const defaultCommitMode: CommitMode = { _tag: "Default" };
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
	mode = defaultSegmentMode,
}: Omit<SegmentItem, "mode"> & { mode?: SegmentMode }): Item => ({
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
	mode = defaultCommitMode,
}: Omit<CommitItem, "mode"> & { mode?: CommitMode }): Item => ({
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

// We intentionally omit state like mode, which affects presentation but not
// identity.
export const itemIdentityKey = (item: Item): string =>
	Match.value(item).pipe(
		Match.tagsExhaustive({
			Changes: (item) => JSON.stringify(["Changes", item.stackId]),
			Change: (item) => JSON.stringify(["Change", item.stackId, item.path]),
			Segment: (item) =>
				JSON.stringify(["Segment", item.stackId, item.segmentIndex, item.branchName]),
			Commit: (item) => JSON.stringify(["Commit", item.stackId, item.segmentIndex, item.commitId]),
			BaseCommit: (item) => JSON.stringify(["BaseCommit", item.commitId]),
		}),
	);

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
