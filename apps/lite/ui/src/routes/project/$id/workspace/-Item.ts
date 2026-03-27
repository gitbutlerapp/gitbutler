import { type RefInfo } from "@gitbutler/but-sdk";
import { Match } from "effect";

type ChangesMode = { _tag: "Summary" } | { _tag: "Details"; path?: string };
export type ChangesItem = { stackId: string | null; mode: ChangesMode };

type SegmentItem = {
	stackId: string;
	segmentIndex: number;
	branchName: string | null;
	branchRef: string | null;
};

export type CommitItem = SegmentItem & { commitId: string };

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

export const commitItem = ({
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
});

export const getParentSection = (selection: Item): Item | null =>
	Match.value(selection).pipe(
		Match.tag("Commit", (item): Item | null => segmentItem(item)),
		Match.tag("Changes", (item): Item | null =>
			item.mode._tag === "Details" ? changesSummaryItem(item.stackId) : null,
		),
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
		Match.exhaustive,
	);

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
