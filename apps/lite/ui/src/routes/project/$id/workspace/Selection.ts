import { type HunkAssignment, type RefInfo, type TreeChange } from "@gitbutler/but-sdk";
import { Match } from "effect";

type CommitDetailSelection = {
	path?: string;
};

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
			detail?: CommitDetailSelection;
	  };

export const isBranchSelected = (
	selection: Selection | null,
	stackId: string,
	branchRef: string,
): boolean =>
	selection?._tag === "Branch" &&
	selection.stackId === stackId &&
	selection.branchRef === branchRef;

export const isChangesFileSelected = (
	selection: Selection | null,
	stackId: string | null,
	path: string,
): boolean =>
	selection?._tag === "ChangesFile" && selection.stackId === stackId && selection.path === path;

export const isCommitSelected = (
	selection: Selection | null,
	stackId: string,
	commitId: string,
): boolean =>
	selection?._tag === "Commit" && selection.stackId === stackId && selection.commitId === commitId;

export const isCommitSelectedAndNotShowingDetails = (
	selection: Selection | null,
	stackId: string,
	commitId: string,
): boolean =>
	selection?._tag === "Commit" &&
	selection.stackId === stackId &&
	selection.commitId === commitId &&
	selection.detail === undefined;

export const isCommitSelectedAndShowingDetails = (
	selection: Selection | null,
	stackId: string,
	commitId: string,
): boolean =>
	selection?._tag === "Commit" &&
	selection.stackId === stackId &&
	selection.commitId === commitId &&
	selection.detail !== undefined;

export const isCommitEditingMessage = (
	selection: Selection | null,
	stackId: string,
	commitId: string,
): boolean =>
	selection?._tag === "Commit" &&
	selection.stackId === stackId &&
	selection.commitId === commitId &&
	selection.detail === undefined &&
	selection.isEditingMessage === true;

export const isCommitFileSelected = (
	selection: Selection | null,
	stackId: string,
	commitId: string,
	path: string,
): boolean =>
	selection?._tag === "Commit" &&
	selection.stackId === stackId &&
	selection.commitId === commitId &&
	selection.detail?.path === path;

export const toggleBranchSelection = (
	selection: Selection | null,
	stackId: string,
	branchName: string,
	branchRef: string,
): Selection | null =>
	isBranchSelected(selection, stackId, branchRef)
		? null
		: { _tag: "Branch", stackId, branchName, branchRef };

export const toggleChangesFileSelection = (
	selection: Selection | null,
	stackId: string | null,
	path: string,
): Selection | null =>
	isChangesFileSelected(selection, stackId, path) ? null : { _tag: "ChangesFile", stackId, path };

export const toggleCommitSelection = (
	selection: Selection | null,
	stackId: string,
	commitId: string,
	branchName: string,
	branchRef: string | null,
): Selection | null =>
	isCommitSelectedAndNotShowingDetails(selection, stackId, commitId)
		? branchRef !== null
			? { _tag: "Branch", stackId, branchName, branchRef }
			: null
		: { _tag: "Commit", stackId, commitId, isEditingMessage: false };

export const toggleCommitEditingMessage = (
	selection: Selection | null,
	stackId: string,
	commitId: string,
): Selection | null => {
	if (isCommitEditingMessage(selection, stackId, commitId))
		return selection?._tag === "Commit" &&
			selection.stackId === stackId &&
			selection.commitId === commitId &&
			selection.detail === undefined &&
			selection.isEditingMessage === true
			? { ...selection, isEditingMessage: false }
			: selection;

	return {
		_tag: "Commit",
		stackId,
		commitId,
		isEditingMessage: true,
		detail: undefined,
	};
};

export const toggleCommitFileSelection = (
	selection: Selection | null,
	stackId: string,
	commitId: string,
	path: string,
): Selection | null =>
	isCommitFileSelected(selection, stackId, commitId, path)
		? {
				_tag: "Commit",
				stackId,
				commitId,
				isEditingMessage: false,
				detail: undefined,
			}
		: {
				_tag: "Commit",
				stackId,
				commitId,
				isEditingMessage: false,
				detail: { path },
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
