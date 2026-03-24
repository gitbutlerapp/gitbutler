import { type HunkAssignment, type RefInfo, type TreeChange } from "@gitbutler/but-sdk";
import { Match } from "effect";

export type Selection =
	| {
			_tag: "Branch";
			stackId: string;
			branchName: string;
			branchRef: string;
	  }
	| {
			_tag: "ChangesFile";
			stackId: string | null;
			path: string;
	  }
	| {
			_tag: "Commit";
			stackId: string;
			commitId: string;
			isEditingMessage?: boolean;
	  }
	| {
			_tag: "CommitFile";
			stackId: string;
			commitId: string;
			path: string;
	  };

export const normalizeSelection = (
	selection: Selection,
	stackIdsByCommitId: Map<string, Set<string>>,
	branchRefsByStackId: Map<string, Set<string>>,
): Selection | null =>
	Match.value(selection).pipe(
		Match.tag("Branch", (selection) => {
			const branchRefs = branchRefsByStackId.get(selection.stackId);
			if (branchRefs === undefined) return null;
			return branchRefs.has(selection.branchRef) ? selection : null;
		}),
		Match.tag("ChangesFile", (selection) => selection),
		Match.tag("Commit", "CommitFile", (selection) => {
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
		return { _tag: "ChangesFile", stackId: null, path: firstUnassignedPath };

	for (const stack of headInfo.stacks) {
		if (stack.id == null) continue;

		const firstAssignedPath = firstSelectablePath({
			changes,
			assignments,
			stackId: stack.id,
		});
		if (firstAssignedPath !== null)
			return {
				_tag: "ChangesFile",
				stackId: stack.id,
				path: firstAssignedPath,
			};

		for (const segment of stack.segments) {
			const firstCommit = segment.commits[0];
			if (firstCommit) return { _tag: "Commit", stackId: stack.id, commitId: firstCommit.id };
		}
	}

	return null;
};
