import { Match } from "effect";

export type ChangesSectionItem = { stackId: string | null };
export type ChangeItem = ChangesSectionItem & { path: string };

type SegmentItemBase = {
	stackId: string;
	segmentIndex: number;
	branchName: string | null;
};

export type SegmentItem = SegmentItemBase;
export type CommitItem = SegmentItemBase & { commitId: string };

export type BaseCommitItem = { commitId: string };

export type Item =
	| ({ _tag: "ChangesSection" } & ChangesSectionItem)
	| ({ _tag: "Change" } & ChangeItem)
	| ({ _tag: "Segment" } & SegmentItem)
	| ({ _tag: "Commit" } & CommitItem)
	| ({ _tag: "BaseCommit" } & BaseCommitItem);

export const changesSectionItem = (stackId: string | null): Item => ({
	_tag: "ChangesSection",
	stackId,
});

export const changeItem = (stackId: string | null, path: string): Item => ({
	_tag: "Change",
	stackId,
	path,
});

export const segmentItem = ({ stackId, segmentIndex, branchName }: SegmentItem): Item => ({
	_tag: "Segment",
	stackId,
	segmentIndex,
	branchName,
});

export const commitItem = ({ stackId, segmentIndex, branchName, commitId }: CommitItem): Item => ({
	_tag: "Commit",
	stackId,
	segmentIndex,
	branchName,
	commitId,
});

export const baseCommitItem = (commitId: string): Item => ({
	_tag: "BaseCommit",
	commitId,
});

export const itemIdentityKey = (item: Item): string =>
	Match.value(item).pipe(
		Match.tagsExhaustive({
			ChangesSection: (item) => JSON.stringify(["ChangesSection", item.stackId]),
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
			ChangesSection: () => null,
			BaseCommit: () => null,
			Segment: () => null,
		}),
	);

export const getStackId = (item: Item): string | null =>
	Match.value(item).pipe(
		Match.tagsExhaustive({
			ChangesSection: (item) => item.stackId,
			Change: (item) => item.stackId,
			Commit: (item) => item.stackId,
			BaseCommit: () => null,
			Segment: (item) => item.stackId,
		}),
	);
