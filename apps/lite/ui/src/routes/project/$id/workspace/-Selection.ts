import { type HunkAssignment, type RefInfo, type TreeChange } from "@gitbutler/but-sdk";
import { Match } from "effect";

type ChangesMode = { _tag: "Summary" } | { _tag: "Details"; path?: string };
type ChangesSelection = { stackId: string | null; mode: ChangesMode };

type SegmentSelection = {
	stackId: string;
	segmentIndex: number;
	branchName: string | null;
	branchRef: string | null;
};

type CommitMode =
	| { _tag: "Summary" }
	| { _tag: "Details"; path?: string }
	| { _tag: "EditingMessage" };
type CommitSelection = SegmentSelection & { commitId: string; mode: CommitMode };

export type Selection =
	| ({ _tag: "Changes" } & ChangesSelection)
	| ({ _tag: "Segment" } & SegmentSelection)
	| ({ _tag: "Commit" } & CommitSelection);

export const changesSummarySelection = (stackId: string | null): Selection => ({
	_tag: "Changes",
	stackId,
	mode: { _tag: "Summary" },
});

export const changesDetailsSelection = (stackId: string | null, path?: string): Selection => ({
	_tag: "Changes",
	stackId,
	mode: { _tag: "Details", path },
});

export const segmentSelection = ({
	stackId,
	segmentIndex,
	branchName,
	branchRef,
}: SegmentSelection): Selection => ({
	_tag: "Segment",
	stackId,
	segmentIndex,
	branchName,
	branchRef,
});

export const commitSummarySelection = ({
	stackId,
	segmentIndex,
	branchName,
	branchRef,
	commitId,
}: Omit<CommitSelection, "mode">): Selection => ({
	_tag: "Commit",
	stackId,
	segmentIndex,
	branchName,
	branchRef,
	commitId,
	mode: { _tag: "Summary" },
});

export const commitDetailsSelection = (
	{ stackId, segmentIndex, branchName, branchRef, commitId }: Omit<CommitSelection, "mode">,
	path?: string,
): Selection => ({
	_tag: "Commit",
	stackId,
	segmentIndex,
	branchName,
	branchRef,
	commitId,
	mode: { _tag: "Details", path },
});

export const commitEditingMessageSelection = ({
	stackId,
	segmentIndex,
	branchName,
	branchRef,
	commitId,
}: Omit<CommitSelection, "mode">): Selection => ({
	_tag: "Commit",
	stackId,
	segmentIndex,
	branchName,
	branchRef,
	commitId,
	mode: { _tag: "EditingMessage" },
});

export const toggleChangesSelection = (
	selection: Selection | null,
	stackId: string | null,
): Selection | null =>
	selection?._tag === "Changes" &&
	selection.stackId === stackId &&
	selection.mode._tag !== "Details"
		? null
		: changesSummarySelection(stackId);

export const toggleSegmentSelection = (
	selection: Selection | null,
	stackId: string,
	segmentIndex: number,
	branchName: string | null,
	branchRef: string | null,
): Selection | null =>
	selection?._tag === "Segment" &&
	selection.stackId === stackId &&
	selection.segmentIndex === segmentIndex
		? null
		: segmentSelection({ stackId, segmentIndex, branchName, branchRef });

export const toggleChangesFileSelection = (
	selection: Selection | null,
	stackId: string | null,
	path: string,
): Selection | null =>
	selection?._tag === "Changes" &&
	selection.stackId === stackId &&
	selection.mode._tag === "Details" &&
	selection.mode.path === path
		? changesSummarySelection(stackId)
		: changesDetailsSelection(stackId, path);

export const toggleCommitSelection = (
	selection: Selection | null,
	stackId: string,
	segmentIndex: number,
	commitId: string,
	branchName: string | null,
	branchRef: string | null,
): Selection | null =>
	selection?._tag === "Commit" &&
	selection.stackId === stackId &&
	selection.commitId === commitId &&
	selection.mode._tag !== "Details"
		? segmentSelection({ stackId, segmentIndex, branchName, branchRef })
		: commitSummarySelection({ stackId, segmentIndex, branchName, branchRef, commitId });

export const toggleCommitEditingMessage = (
	selection: Selection | null,
	stackId: string,
	segmentIndex: number,
	branchName: string | null,
	branchRef: string | null,
	commitId: string,
): Selection | null =>
	selection?._tag === "Commit" &&
	selection.stackId === stackId &&
	selection.commitId === commitId &&
	selection.mode._tag === "EditingMessage"
		? {
				...selection,
				mode: { _tag: "Summary" },
			}
		: commitEditingMessageSelection({ stackId, segmentIndex, branchName, branchRef, commitId });

export const toggleCommitFileSelection = (
	selection: Selection | null,
	stackId: string,
	segmentIndex: number,
	branchName: string | null,
	branchRef: string | null,
	commitId: string,
	path: string,
): Selection | null =>
	selection?._tag === "Commit" &&
	selection.stackId === stackId &&
	selection.commitId === commitId &&
	selection.mode._tag === "Details" &&
	selection.mode.path === path
		? commitSummarySelection({ stackId, segmentIndex, branchName, branchRef, commitId })
		: commitDetailsSelection({ stackId, segmentIndex, branchName, branchRef, commitId }, path);

export const normalizeSelection = (selection: Selection, headInfo: RefInfo): Selection | null =>
	Match.value(selection).pipe(
		Match.tag("Changes", (selection) => selection),
		Match.tag("Segment", (selection) => {
			const stack = headInfo.stacks.find(
				(stack) => stack.id !== null && stack.id === selection.stackId,
			);
			if (!stack) return null;
			const segment = stack.segments[selection.segmentIndex];
			if (!segment) return null;
			return selection;
		}),
		Match.tag("Commit", (selection) => {
			const stack = headInfo.stacks.find(
				(stack) => stack.id !== null && stack.id === selection.stackId,
			);
			if (!stack) return null;
			const segment = stack.segments[selection.segmentIndex];
			if (!segment) return null;
			if (!segment.commits.some((commit) => commit.id === selection.commitId)) return null;
			return selection;
		}),
		Match.exhaustive,
	);

const hasAssignmentsForPath = ({
	assignments,
	stackId,
	path,
}: {
	assignments: Array<HunkAssignment>;
	stackId: string | null;
	path: string;
}): boolean =>
	assignments.some(
		(assignment) => (assignment.stackId ?? null) === stackId && assignment.path === path,
	);

const firstSelectablePath = ({
	changes,
	assignments,
	stackId,
}: {
	changes: Array<TreeChange>;
	assignments: Array<HunkAssignment>;
	stackId: string | null;
}): string | null =>
	changes.find((change) => hasAssignmentsForPath({ assignments, stackId, path: change.path }))
		?.path ?? null;

export const getDefaultSelection = ({
	headInfo,
	changes,
	assignments,
}: {
	headInfo: RefInfo;
	changes: Array<TreeChange>;
	assignments: Array<HunkAssignment>;
}): Selection | null => {
	const firstUnassignedPath = firstSelectablePath({
		changes,
		assignments,
		stackId: null,
	});
	if (firstUnassignedPath !== null) return changesDetailsSelection(null, firstUnassignedPath);

	for (const stack of headInfo.stacks) {
		if (stack.id == null) continue;

		const firstAssignedPath = firstSelectablePath({
			changes,
			assignments,
			stackId: stack.id,
		});
		if (firstAssignedPath !== null) return changesDetailsSelection(stack.id, firstAssignedPath);

		for (const segment of stack.segments) {
			const firstCommit = segment.commits[0];
			if (firstCommit)
				return commitSummarySelection({
					stackId: stack.id,
					segmentIndex: stack.segments.indexOf(segment),
					branchName: segment.refName?.displayName ?? null,
					branchRef: segment.refName ? `refs/heads/${segment.refName.displayName}` : null,
					commitId: firstCommit.id,
				});
		}
	}

	return null;
};
