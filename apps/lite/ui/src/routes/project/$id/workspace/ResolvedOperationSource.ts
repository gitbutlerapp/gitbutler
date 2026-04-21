import { changeFileParent, commitFileParent, type FileParent } from "#ui/domain/FileParent.ts";
import type { HunkHeader, TreeChange } from "@gitbutler/but-sdk";
import { Match } from "effect";
import { Item } from "#ui/routes/project/$id/workspace/Item.ts";

export type TreeChangeWithHunkHeaders = {
	change: TreeChange;
	hunkHeaders: Array<HunkHeader>;
};

/** @public */
export type CommitResolvedOperationSource = { commitId: string };
/** @public */
export type StackResolvedOperationSource = { stackId: string };
/** @public */
export type BranchResolvedOperationSource = { branchRef: Array<number> };
/** @public */
export type TreeChangesResolvedOperationSource = {
	parent: FileParent;
	changes: Array<TreeChangeWithHunkHeaders>;
};

/**
 * The source of an operation in a form that can be sent to the backend.
 */
export type ResolvedOperationSource =
	| { _tag: "BaseCommit" }
	| ({ _tag: "Commit" } & CommitResolvedOperationSource)
	| ({ _tag: "Stack" } & StackResolvedOperationSource)
	| ({ _tag: "Branch" } & BranchResolvedOperationSource)
	| ({ _tag: "TreeChanges" } & TreeChangesResolvedOperationSource);

/** @public */
export const baseCommitResolvedOperationSource: ResolvedOperationSource = {
	_tag: "BaseCommit",
};

/** @public */
export const commitResolvedOperationSource = ({
	commitId,
}: CommitResolvedOperationSource): ResolvedOperationSource => ({
	_tag: "Commit",
	commitId,
});

/** @public */
export const stackResolvedOperationSource = ({
	stackId,
}: StackResolvedOperationSource): ResolvedOperationSource => ({
	_tag: "Stack",
	stackId,
});

/** @public */
export const branchResolvedOperationSource = ({
	branchRef,
}: BranchResolvedOperationSource): ResolvedOperationSource => ({
	_tag: "Branch",
	branchRef,
});

/** @public */
export const treeChangesResolvedOperationSource = ({
	parent,
	changes,
}: TreeChangesResolvedOperationSource): ResolvedOperationSource => ({
	_tag: "TreeChanges",
	parent,
	changes,
});

export const resolveOperationSource = (item: Item): ResolvedOperationSource =>
	Match.value(item).pipe(
		Match.tagsExhaustive({
			BaseCommit: () => baseCommitResolvedOperationSource,
			Branch: ({ branchRef }) => branchResolvedOperationSource({ branchRef }),
			ChangeFile: ({ treeChange }) =>
				treeChangesResolvedOperationSource({
					parent: changeFileParent,
					changes: [{ change: treeChange, hunkHeaders: [] }],
				}),
			ChangesSection: ({ treeChanges }) =>
				treeChangesResolvedOperationSource({
					parent: changeFileParent,
					changes: treeChanges.map((change) => ({ change, hunkHeaders: [] })),
				}),
			Commit: ({ commitId }) => commitResolvedOperationSource({ commitId }),
			CommitFile: ({ commitId, treeChange }) =>
				treeChangesResolvedOperationSource({
					parent: commitFileParent({ commitId }),
					changes: [{ change: treeChange, hunkHeaders: [] }],
				}),
			Stack: ({ stackId }) => stackResolvedOperationSource({ stackId }),
			Hunk: ({ parent, treeChange }) =>
				treeChangesResolvedOperationSource({
					parent,
					changes: [treeChange],
				}),
		}),
	);
