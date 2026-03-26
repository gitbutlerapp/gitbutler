import { type HunkAssignment, type RefInfo, type TreeChange } from "@gitbutler/but-sdk";
import { Match } from "effect";

type ChangesMode = { _tag: "Summary" } | { _tag: "Details"; path?: string };
type ChangesSelection = { stackId: string | null; mode: ChangesMode };

type BranchSelection = { stackId: string; branchName: string; branchRef: string };

type CommitMode =
	| { _tag: "Summary" }
	| { _tag: "Details"; path?: string }
	| { _tag: "EditingMessage" };
type CommitSelection = { stackId: string; commitId: string; mode: CommitMode };

export type Selection =
	| ({ _tag: "Changes" } & ChangesSelection)
	| ({ _tag: "Branch" } & BranchSelection)
	| ({ _tag: "Commit" } & CommitSelection);

export const toggleChangesSelection = (
	selection: Selection | null,
	stackId: string | null,
): Selection | null =>
	selection?._tag === "Changes" &&
	selection.stackId === stackId &&
	selection.mode._tag !== "Details"
		? null
		: {
				_tag: "Changes",
				stackId,
				mode: { _tag: "Summary" },
			};

export const toggleBranchSelection = (
	selection: Selection | null,
	stackId: string,
	branchName: string,
	branchRef: string,
): Selection | null =>
	selection?._tag === "Branch" &&
	selection.stackId === stackId &&
	selection.branchName === branchName
		? null
		: {
				_tag: "Branch",
				stackId,
				branchName,
				branchRef,
			};

export const toggleChangesFileSelection = (
	selection: Selection | null,
	stackId: string | null,
	path: string,
): Selection | null =>
	selection?._tag === "Changes" &&
	selection.stackId === stackId &&
	selection.mode._tag === "Details" &&
	selection.mode.path === path
		? {
				_tag: "Changes",
				stackId,
				mode: { _tag: "Summary" },
			}
		: {
				_tag: "Changes",
				stackId,
				mode: { _tag: "Details", path },
			};

export const toggleCommitSelection = (
	selection: Selection | null,
	stackId: string,
	commitId: string,
	branchName: string,
	branchRef: string | null,
): Selection | null =>
	selection?._tag === "Commit" &&
	selection.stackId === stackId &&
	selection.commitId === commitId &&
	selection.mode._tag !== "Details"
		? branchRef !== null
			? {
					_tag: "Branch",
					stackId,
					branchName,
					branchRef,
				}
			: null
		: {
				_tag: "Commit",
				stackId,
				commitId,
				mode: { _tag: "Summary" },
			};

export const toggleCommitEditingMessage = (
	selection: Selection | null,
	stackId: string,
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
		: {
				_tag: "Commit",
				stackId,
				commitId,
				mode: { _tag: "EditingMessage" },
			};

export const toggleCommitFileSelection = (
	selection: Selection | null,
	stackId: string,
	commitId: string,
	path: string,
): Selection | null =>
	selection?._tag === "Commit" &&
	selection.stackId === stackId &&
	selection.commitId === commitId &&
	selection.mode._tag === "Details" &&
	selection.mode.path === path
		? {
				_tag: "Commit",
				stackId,
				commitId,
				mode: { _tag: "Summary" },
			}
		: {
				_tag: "Commit",
				stackId,
				commitId,
				mode: { _tag: "Details", path },
			};

export const normalizeSelection = (
	selection: Selection,
	stackIdsByCommitId: Map<string, Set<string>>,
	branchRefsByStackId: Map<string, Set<string>>,
): Selection | null =>
	Match.value(selection).pipe(
		Match.tag("Changes", (selection) => selection),
		Match.tag("Branch", (selection) => {
			const branchRefs = branchRefsByStackId.get(selection.stackId);
			if (branchRefs === undefined) return null;
			return branchRefs.has(selection.branchRef) ? selection : null;
		}),
		Match.tag("Commit", (selection) => {
			const stackIds = stackIdsByCommitId.get(selection.commitId);
			if (stackIds === undefined) return null;
			if (!stackIds.has(selection.stackId)) return null;
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
	if (firstUnassignedPath !== null)
		return {
			_tag: "Changes",
			stackId: null,
			mode: { _tag: "Details", path: firstUnassignedPath },
		};

	for (const stack of headInfo.stacks) {
		if (stack.id == null) continue;

		const firstAssignedPath = firstSelectablePath({
			changes,
			assignments,
			stackId: stack.id,
		});
		if (firstAssignedPath !== null)
			return {
				_tag: "Changes",
				stackId: stack.id,
				mode: { _tag: "Details", path: firstAssignedPath },
			};

		for (const segment of stack.segments) {
			const firstCommit = segment.commits[0];
			if (firstCommit)
				return {
					_tag: "Commit",
					stackId: stack.id,
					commitId: firstCommit.id,
					mode: { _tag: "Summary" },
				};
		}
	}

	return null;
};
