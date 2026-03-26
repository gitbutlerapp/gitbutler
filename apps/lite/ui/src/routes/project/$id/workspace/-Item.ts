import { type RefInfo } from "@gitbutler/but-sdk";
import { Match } from "effect";

type ChangesMode = { _tag: "Summary" } | { _tag: "Details"; path?: string };
type ChangesItem = { stackId: string | null; mode: ChangesMode };

type SegmentItem = {
	stackId: string;
	segmentIndex: number;
	branchName: string | null;
	branchRef: string | null;
};

type CommitMode =
	| { _tag: "Summary" }
	| { _tag: "Details"; path?: string }
	| { _tag: "EditingMessage" };
type CommitItem = SegmentItem & { commitId: string; mode: CommitMode };

export type Item =
	| ({ _tag: "Changes" } & ChangesItem)
	| ({ _tag: "Segment" } & SegmentItem)
	| ({ _tag: "Commit" } & CommitItem);

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

export const commitSummaryItem = ({
	stackId,
	segmentIndex,
	branchName,
	branchRef,
	commitId,
}: Omit<CommitItem, "mode">): Item => ({
	_tag: "Commit",
	stackId,
	segmentIndex,
	branchName,
	branchRef,
	commitId,
	mode: { _tag: "Summary" },
});

export const commitDetailsItem = (
	{ stackId, segmentIndex, branchName, branchRef, commitId }: Omit<CommitItem, "mode">,
	path?: string,
): Item => ({
	_tag: "Commit",
	stackId,
	segmentIndex,
	branchName,
	branchRef,
	commitId,
	mode: { _tag: "Details", path },
});

export const commitEditingMessageItem = ({
	stackId,
	segmentIndex,
	branchName,
	branchRef,
	commitId,
}: Omit<CommitItem, "mode">): Item => ({
	_tag: "Commit",
	stackId,
	segmentIndex,
	branchName,
	branchRef,
	commitId,
	mode: { _tag: "EditingMessage" },
});

export const getContainerItem = (selection: Item): Item | null =>
	Match.value(selection).pipe(
		Match.tag("Commit", (item): Item | null => segmentItem(item)),
		Match.tag("Changes", (item): Item | null =>
			item.mode._tag === "Details" ? changesSummaryItem(item.stackId) : null,
		),
		Match.tag("Segment", () => null),
		Match.exhaustive,
	);

export const getParentItem = (item: Item): Item | null =>
	Match.value(item).pipe(
		Match.tag("Commit", (item): Item | null =>
			item.mode._tag === "Details" && item.mode.path !== undefined
				? commitSummaryItem(item)
				: segmentItem(item),
		),
		Match.tag("Changes", (item): Item | null =>
			item.mode._tag === "Details" ? changesSummaryItem(item.stackId) : null,
		),
		Match.tag("Segment", () => null),
		Match.exhaustive,
	);

export const itemsEqual = (a: Item | null, b: Item | null): boolean => {
	if (a === null || b === null) return a === b;
	if (a._tag !== b._tag) return false;

	return Match.value(a).pipe(
		Match.tag(
			"Changes",
			(a) =>
				b._tag === "Changes" &&
				a.stackId === b.stackId &&
				a.mode._tag === b.mode._tag &&
				(a.mode._tag !== "Details" || (b.mode._tag === "Details" && a.mode.path === b.mode.path)),
		),
		Match.tag(
			"Segment",
			(a) => b._tag === "Segment" && a.stackId === b.stackId && a.segmentIndex === b.segmentIndex,
		),
		Match.tag(
			"Commit",
			(a) =>
				b._tag === "Commit" &&
				a.stackId === b.stackId &&
				a.segmentIndex === b.segmentIndex &&
				a.commitId === b.commitId &&
				a.mode._tag === b.mode._tag &&
				(a.mode._tag !== "Details" || (b.mode._tag === "Details" && a.mode.path === b.mode.path)),
		),
		Match.exhaustive,
	);
};

export const normalizeItem = (item: Item, headInfo: RefInfo): Item | null =>
	Match.value(item).pipe(
		Match.tag("Changes", (item) => item),
		Match.tag("Segment", (item) => {
			const stack = headInfo.stacks.find((stack) => stack.id !== null && stack.id === item.stackId);
			if (!stack) return null;
			const segment = stack.segments[item.segmentIndex];
			if (!segment) return null;
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
		Match.exhaustive,
	);
