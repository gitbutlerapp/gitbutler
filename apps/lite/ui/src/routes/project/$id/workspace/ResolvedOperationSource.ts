import {
	changesInWorktreeQueryOptions,
	commitDetailsWithLineStatsQueryOptions,
} from "#ui/api/queries.ts";
import {
	commitAmendOperation,
	commitCreateFromCommittedChangesOperation,
	commitCreateOperation,
	commitMoveChangesBetweenOperation,
	commitMoveOperation,
	commitSquashOperation,
	commitUncommitChangesOperation,
	commitUncommitOperation,
	moveBranchOperation,
	tearOffBranchOperation,
	type Operation,
} from "#ui/Operation.ts";
import { createDiffSpec } from "#ui/domain/DiffSpec.ts";
import { changeFileParent, commitFileParent, type FileParent } from "#ui/domain/FileParent.ts";
import { QueryClient } from "@tanstack/react-query";
import {
	CommitDetails,
	InsertSide,
	WorktreeChanges,
	type HunkHeader,
	type TreeChange,
} from "@gitbutler/but-sdk";
import { Match } from "effect";
import { decodeRefName } from "../shared";
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
					(change): Array<TreeChangeWithHunkHeaders> => [
						{
							change,
							hunkHeaders: [],
						},
					],
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
				const change = Match.value(parent).pipe(
					Match.tagsExhaustive({
						Change: () => {
							if (!worktreeChanges) return null;

							return worktreeChanges.changes.find((candidate) => candidate.path === path) ?? null;
						},
						Commit: ({ commitId }) => {
							const commitDetails = getCommitDetails(commitId);
							if (!commitDetails) return null;

							return commitDetails.changes.find((candidate) => candidate.path === path) ?? null;
						},
					}),
				);

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

const getCommitTargetMoveOperation = ({
	resolvedOperationSource,
	commitId,
	side,
}: {
	resolvedOperationSource: ResolvedOperationSource;
	commitId: string;
	side: InsertSide;
}) =>
	Match.value(resolvedOperationSource).pipe(
		Match.tags({
			Commit: (source) =>
				commitMoveOperation({
					subjectCommitIds: [source.commitId],
					relativeTo: { type: "commit", subject: commitId },
					side,
					dryRun: false,
				}),
			TreeChanges: (source) => {
				const changes = source.changes.map(({ change, hunkHeaders }) =>
					createDiffSpec(change, hunkHeaders),
				);

				return Match.value(source.parent).pipe(
					Match.tagsExhaustive({
						Change: () =>
							commitCreateOperation({
								relativeTo: { type: "commit", subject: commitId },
								side,
								changes,
								message: "",
								dryRun: false,
							}),
						Commit: (source) =>
							commitCreateFromCommittedChangesOperation({
								sourceCommitId: source.commitId,
								relativeTo: { type: "commit", subject: commitId },
								side,
								changes,
								dryRun: false,
							}),
					}),
				);
			},
		}),
		Match.orElse(() => null),
	);

const getBranchTargetOperation = ({
	resolvedOperationSource,
	branchRef,
}: {
	resolvedOperationSource: ResolvedOperationSource;
	branchRef: Array<number>;
}): Operation | null =>
	Match.value(resolvedOperationSource).pipe(
		Match.tags({
			Branch: (source) =>
				moveBranchOperation({
					subjectBranch: decodeRefName(source.branchRef),
					targetBranch: decodeRefName(branchRef),
					dryRun: false,
				}),
			Commit: ({ commitId }) =>
				commitMoveOperation({
					subjectCommitIds: [commitId],
					relativeTo: {
						type: "referenceBytes",
						subject: branchRef,
					},
					side: "below",
					dryRun: false,
				}),
			TreeChanges: (source) => {
				const changes = source.changes.map(({ change, hunkHeaders }) =>
					createDiffSpec(change, hunkHeaders),
				);

				return Match.value(source.parent).pipe(
					Match.tagsExhaustive({
						Change: () =>
							commitCreateOperation({
								relativeTo: { type: "referenceBytes", subject: branchRef },
								side: "below",
								changes,
								message: "",
								dryRun: false,
							}),
						Commit: (source) =>
							commitCreateFromCommittedChangesOperation({
								sourceCommitId: source.commitId,
								relativeTo: { type: "referenceBytes", subject: branchRef },
								side: "below",
								changes,
								dryRun: false,
							}),
					}),
				);
			},
		}),
		Match.orElse(() => null),
	);

const getTearOffBranchTargetOperation = (
	resolvedOperationSource: ResolvedOperationSource,
): Operation | null => {
	if (resolvedOperationSource._tag !== "Branch") return null;

	return tearOffBranchOperation({
		subjectBranch: decodeRefName(resolvedOperationSource.branchRef),
		dryRun: false,
	});
};

/**
 * | SOURCE ↓ / TARGET →    | Changes  | Commit |
 * | ---------------------- | -------- | ------ |
 * | File/hunk from changes | No-op    | Amend  |
 * | File/hunk from commit  | Uncommit | Amend  |
 * | Commit                 | Uncommit | Squash |
 *
 * Note this is currently different from the CLI's definition of "rubbing",
 * which also includes move operations.
 * https://linear.app/gitbutler/issue/GB-1160/what-should-rubbing-a-branch-into-another-branch-do#comment-db2abdb7
 */
export const rubOperationSourceToOperation = ({
	resolvedOperationSource,
	target,
}: {
	resolvedOperationSource: ResolvedOperationSource;
	target: Item;
}): Operation | null =>
	Match.value(resolvedOperationSource).pipe(
		Match.tagsExhaustive({
			Stack: () => null,
			Branch: () => null,
			BaseCommit: () => null,
			Commit: (source) =>
				Match.value(target).pipe(
					Match.tags({
						ChangesSection: () =>
							commitUncommitOperation({
								commitId: source.commitId,
								assignTo: null,
							}),
						Commit: (target) =>
							commitSquashOperation({
								sourceCommitId: source.commitId,
								destinationCommitId: target.commitId,
								dryRun: false,
							}),
					}),
					Match.orElse(() => null),
				),
			TreeChanges: (source) => {
				const changes = source.changes.map(({ change, hunkHeaders }) =>
					createDiffSpec(change, hunkHeaders),
				);

				return Match.value(source.parent).pipe(
					Match.tagsExhaustive({
						Change: () =>
							Match.value(target).pipe(
								Match.tags({
									ChangesSection: () => null,
									Commit: ({ commitId }) =>
										commitAmendOperation({
											commitId,
											changes,
											dryRun: false,
										}),
								}),
								Match.orElse(() => null),
							),
						Commit: (source) =>
							Match.value(target).pipe(
								Match.tags({
									ChangesSection: () =>
										commitUncommitChangesOperation({
											commitId: source.commitId,
											assignTo: null,
											changes,
											dryRun: false,
										}),
									Commit: (target) =>
										commitMoveChangesBetweenOperation({
											sourceCommitId: source.commitId,
											destinationCommitId: target.commitId,
											changes,
											dryRun: false,
										}),
								}),
								Match.orElse(() => null),
							),
					}),
				);
			},
		}),
	);

export const moveOperationSourceToOperation = ({
	resolvedOperationSource,
	target,
	side,
}: {
	resolvedOperationSource: ResolvedOperationSource;
	target: Item;
	side: InsertSide;
}) =>
	Match.value(target).pipe(
		Match.tags({
			Branch: ({ branchRef }) =>
				getBranchTargetOperation({
					resolvedOperationSource,
					branchRef,
				}),
			Commit: (target) =>
				getCommitTargetMoveOperation({
					resolvedOperationSource,
					commitId: target.commitId,
					side,
				}),
			BaseCommit: () => getTearOffBranchTargetOperation(resolvedOperationSource),
		}),
		Match.orElse(() => null),
	);
