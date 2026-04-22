import {
	changesInWorktreeQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
} from "#ui/api/queries.ts";
import { changeFileParent, commitFileParent, type FileParent } from "#ui/domain/FileParent.ts";
import { QueryClient } from "@tanstack/react-query";
import {
	CommitDetails,
	WorktreeChanges,
	type HunkHeader,
	type TreeChange,
} from "@gitbutler/but-sdk";
import { Match } from "effect";
import { Item } from "#ui/routes/project/$id/workspace/Item.ts";

type TreeChangeWithHunkHeaders = {
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

const resolvedOperationSourceFromItem = ({
	item,
	worktreeChanges,
	getCommitDetails,
}: {
	item: Item;
	worktreeChanges: WorktreeChanges | undefined;
	getCommitDetails: (commitId: string) => CommitDetails | undefined;
}) =>
	Match.value(item).pipe(
		Match.tagsExhaustive({
			BaseCommit: () => baseCommitResolvedOperationSource,
			Branch: ({ branchRef }) => branchResolvedOperationSource({ branchRef }),
			ChangeFile: ({ path }) => {
				if (!worktreeChanges) return null;

				const change = worktreeChanges.changes.find((candidate) => candidate.path === path);
				if (!change) return null;

				return treeChangesResolvedOperationSource({
					parent: changeFileParent,
					changes: [{ change, hunkHeaders: [] }],
				});
			},
			ChangesSection: () => {
				if (!worktreeChanges) return null;

				const changes = worktreeChanges.changes.flatMap(
					(change): Array<TreeChangeWithHunkHeaders> => [{ change, hunkHeaders: [] }],
				);
				return treeChangesResolvedOperationSource({
					parent: changeFileParent,
					changes,
				});
			},
			Commit: ({ commitId }) => commitResolvedOperationSource({ commitId }),
			CommitFile: ({ commitId, path }) => {
				const commitDetails = getCommitDetails(commitId);
				if (!commitDetails) return null;

				const change = commitDetails.changes.find((candidate) => candidate.path === path);
				if (!change) return null;

				return treeChangesResolvedOperationSource({
					parent: commitFileParent({ commitId }),
					changes: [{ change, hunkHeaders: [] }],
				});
			},
			Stack: ({ stackId }) => stackResolvedOperationSource({ stackId }),
			Hunk: ({ parent, path, hunkHeader }) => {
				const changes = Match.value(parent).pipe(
					Match.tagsExhaustive({
						Change: () => {
							if (!worktreeChanges) return null;
							return worktreeChanges.changes;
						},
						Commit: ({ commitId }) => {
							const commitDetails = getCommitDetails(commitId);
							if (!commitDetails) return null;
							return commitDetails.changes;
						},
					}),
				);
				if (!changes) return null;

				const change = changes.find((candidate) => candidate.path === path);
				if (!change) return null;

				return treeChangesResolvedOperationSource({
					parent,
					changes: [{ change, hunkHeaders: [hunkHeader] }],
				});
			},
		}),
	);

export const resolveOperationSource = ({
	operationSource,
	queryClient,
	projectId,
}: {
	operationSource: Item;
	queryClient: QueryClient;
	projectId: string;
}) =>
	resolvedOperationSourceFromItem({
		item: operationSource,
		worktreeChanges: queryClient.getQueryData(changesInWorktreeQueryOptions(projectId).queryKey),
		getCommitDetails: (commitId) =>
			queryClient.getQueryData(
				commitDetailsWithLineStatsQueryOptions({ projectId, commitId }).queryKey,
			),
	});
