import { Toast } from "@base-ui/react";
import { useMutation } from "@tanstack/react-query";
import { Match } from "effect";
import {
	type CommitAmendParams,
	type CommitCreateParams,
	type CommitInsertBlankParams,
	type CommitMoveParams,
	type CommitMoveChangesBetweenParams,
	type CommitUncommitChangesParams,
	type MoveBranchParams,
	type TearOffBranchParams,
	CommitSquashParams,
} from "#electron/ipc.ts";
import { rejectedChangesToastOptions } from "#ui/RejectedChanges.tsx";
import {
	commitAmendMutationOptions,
	commitCreateMutationOptions,
	commitInsertBlankMutationOptions,
	commitMoveMutationOptions,
	commitMoveChangesBetweenMutationOptions,
	commitSquashMutationOptions,
	commitUncommitChangesMutationOptions,
	commitUncommitMutationOptions,
	moveBranchMutationOptions,
	tearOffBranchMutationOptions,
	CommitUncommitParams,
} from "#ui/api/mutations.ts";
import { InsertSide, RelativeTo } from "@gitbutler/but-sdk";
import { ResolvedOperationSource } from "#ui/routes/project/$id/workspace/ResolvedOperationSource.ts";
import { createDiffSpec } from "#ui/domain/DiffSpec.ts";
import { decodeRefName } from "#ui/routes/project/$id/shared.tsx";
import { Item } from "#ui/routes/project/$id/workspace/Item.ts";

/** @public */
export type CommitAmendOperation = Omit<CommitAmendParams, "projectId">;
/** @public */
export type CommitCreateOperation = Omit<CommitCreateParams, "projectId">;
/** @public */
export type CommitCreateFromCommittedChangesOperation = Omit<CommitInsertBlankParams, "projectId"> &
	Pick<CommitMoveChangesBetweenParams, "changes" | "sourceCommitId">;
/** @public */
export type CommitMoveOperation = Omit<CommitMoveParams, "projectId">;
/** @public */
export type CommitMoveChangesBetweenOperation = Omit<CommitMoveChangesBetweenParams, "projectId">;
/** @public */
export type CommitSquashOperation = Omit<CommitSquashParams, "projectId">;
/** @public */
export type CommitUncommitOperation = Omit<CommitUncommitParams, "projectId">;
/** @public */
export type CommitUncommitChangesOperation = Omit<CommitUncommitChangesParams, "projectId">;
/** @public */
export type MoveBranchOperation = Omit<MoveBranchParams, "projectId">;
/** @public */
export type TearOffBranchOperation = Omit<TearOffBranchParams, "projectId">;

export type Operation =
	| ({ _tag: "CommitAmend" } & CommitAmendOperation)
	| ({ _tag: "CommitCreate" } & CommitCreateOperation)
	| ({ _tag: "CommitCreateFromCommittedChanges" } & CommitCreateFromCommittedChangesOperation)
	| ({ _tag: "CommitMove" } & CommitMoveOperation)
	| ({ _tag: "CommitMoveChangesBetween" } & CommitMoveChangesBetweenOperation)
	| ({ _tag: "CommitSquash" } & CommitSquashOperation)
	| ({ _tag: "CommitUncommit" } & CommitUncommitOperation)
	| ({ _tag: "CommitUncommitChanges" } & CommitUncommitChangesOperation)
	| ({ _tag: "MoveBranch" } & MoveBranchOperation)
	| ({ _tag: "TearOffBranch" } & TearOffBranchOperation);

/** @public */
export const commitAmendOperation = (operation: CommitAmendOperation): Operation => ({
	_tag: "CommitAmend",
	...operation,
});

/** @public */
export const commitCreateOperation = (operation: CommitCreateOperation): Operation => ({
	_tag: "CommitCreate",
	...operation,
});

/** @public */
export const commitCreateFromCommittedChangesOperation = (
	operation: CommitCreateFromCommittedChangesOperation,
): Operation => ({
	_tag: "CommitCreateFromCommittedChanges",
	...operation,
});

/** @public */
export const commitMoveOperation = (operation: CommitMoveOperation): Operation => ({
	_tag: "CommitMove",
	...operation,
});

/** @public */
export const commitMoveChangesBetweenOperation = (
	operation: CommitMoveChangesBetweenOperation,
): Operation => ({
	_tag: "CommitMoveChangesBetween",
	...operation,
});

/** @public */
export const commitSquashOperation = (operation: CommitSquashOperation): Operation => ({
	_tag: "CommitSquash",
	...operation,
});

/** @public */
export const commitUncommitOperation = (operation: CommitUncommitOperation): Operation => ({
	_tag: "CommitUncommit",
	...operation,
});

/** @public */
export const commitUncommitChangesOperation = (
	operation: CommitUncommitChangesOperation,
): Operation => ({
	_tag: "CommitUncommitChanges",
	...operation,
});

/** @public */
export const moveBranchOperation = (operation: MoveBranchOperation): Operation => ({
	_tag: "MoveBranch",
	...operation,
});

/** @public */
export const tearOffBranchOperation = (operation: TearOffBranchOperation): Operation => ({
	_tag: "TearOffBranch",
	...operation,
});

export const getInsertionSide = (operation: Operation): InsertSide | null =>
	Match.value(operation).pipe(
		Match.tags({
			CommitMove: (x) => x.side,
			CommitCreate: (x) => x.side,
			CommitCreateFromCommittedChanges: (x) => x.side,
		}),
		Match.orElse(() => null),
	);

export const operationLabel = (operation: Operation): string =>
	Match.value(operation).pipe(
		Match.tagsExhaustive({
			CommitAmend: () => "Amend",
			CommitCreate: ({ side }) =>
				Match.value(side).pipe(
					Match.when("above", () => "Create commit above"),
					Match.when("below", () => "Create commit below"),
					Match.exhaustive,
				),
			CommitCreateFromCommittedChanges: ({ side }) =>
				Match.value(side).pipe(
					Match.when("above", () => "Create commit above"),
					Match.when("below", () => "Create commit below"),
					Match.exhaustive,
				),
			CommitMove: ({ side }) =>
				Match.value(side).pipe(
					Match.when("above", () => "Move commit above"),
					Match.when("below", () => "Move commit below"),
					Match.exhaustive,
				),
			CommitMoveChangesBetween: () => "Amend",
			CommitSquash: () => "Squash",
			CommitUncommit: () => "Uncommit",
			CommitUncommitChanges: () => "Uncommit",
			MoveBranch: () => "Stack branch onto here",
			TearOffBranch: () => "Tear off branch",
		}),
	);

export const useRunOperation = () => {
	const toastManager = Toast.useToastManager();
	const commitAmend = useMutation(commitAmendMutationOptions);
	const commitCreate = useMutation(commitCreateMutationOptions);
	const commitInsertBlank = useMutation(commitInsertBlankMutationOptions);
	const commitMove = useMutation(commitMoveMutationOptions);
	const commitMoveChangesBetween = useMutation(commitMoveChangesBetweenMutationOptions);
	const commitSquash = useMutation(commitSquashMutationOptions);
	const commitUncommit = useMutation(commitUncommitMutationOptions);
	const commitUncommitChanges = useMutation(commitUncommitChangesMutationOptions);
	const moveBranch = useMutation(moveBranchMutationOptions);
	const tearOffBranch = useMutation(tearOffBranchMutationOptions);

	return (projectId: string, operation: Operation): void => {
		Match.value(operation).pipe(
			Match.tagsExhaustive({
				CommitAmend: (operation) => {
					commitAmend.mutate(
						{
							projectId,
							commitId: operation.commitId,
							changes: operation.changes,
							dryRun: operation.dryRun,
						},
						{
							onSuccess: (response) => {
								if (response.rejectedChanges.length > 0)
									toastManager.add(
										rejectedChangesToastOptions({
											newCommit: response.newCommit,
											rejectedChanges: response.rejectedChanges,
										}),
									);
							},
						},
					);
				},
				CommitMoveChangesBetween: (operation) => {
					commitMoveChangesBetween.mutate({
						projectId,
						sourceCommitId: operation.sourceCommitId,
						destinationCommitId: operation.destinationCommitId,
						changes: operation.changes,
						dryRun: operation.dryRun,
					});
				},
				CommitSquash: (operation) => {
					commitSquash.mutate({
						projectId,
						sourceCommitId: operation.sourceCommitId,
						destinationCommitId: operation.destinationCommitId,
						dryRun: operation.dryRun,
					});
				},
				CommitUncommit: (operation) => {
					commitUncommit.mutate({
						projectId,
						commitId: operation.commitId,
						assignTo: operation.assignTo,
					});
				},
				CommitUncommitChanges: (operation) => {
					commitUncommitChanges.mutate({
						projectId,
						commitId: operation.commitId,
						assignTo: operation.assignTo,
						changes: operation.changes,
						dryRun: operation.dryRun,
					});
				},
				CommitCreate: (operation) => {
					commitCreate.mutate(
						{
							projectId,
							relativeTo: operation.relativeTo,
							side: operation.side,
							changes: operation.changes,
							message: operation.message,
							dryRun: operation.dryRun,
						},
						{
							onSuccess: (response) => {
								if (response.rejectedChanges.length > 0)
									toastManager.add(
										rejectedChangesToastOptions({
											newCommit: response.newCommit,
											rejectedChanges: response.rejectedChanges,
										}),
									);
							},
						},
					);
				},
				CommitCreateFromCommittedChanges: (operation) => {
					// Ideally this would be an atomic backend operation.
					void (async () => {
						const insertedCommit = await commitInsertBlank.mutateAsync({
							projectId,
							relativeTo: operation.relativeTo,
							side: operation.side,
							dryRun: operation.dryRun,
						});

						await commitMoveChangesBetween.mutateAsync({
							projectId,
							sourceCommitId:
								insertedCommit.workspace.replacedCommits[operation.sourceCommitId] ??
								operation.sourceCommitId,
							destinationCommitId: insertedCommit.newCommit,
							changes: operation.changes,
							dryRun: operation.dryRun,
						});
					})();
				},
				CommitMove: (operation) => {
					commitMove.mutate({
						projectId,
						subjectCommitIds: operation.subjectCommitIds,
						relativeTo: operation.relativeTo,
						side: operation.side,
						dryRun: operation.dryRun,
					});
				},
				MoveBranch: (operation) => {
					moveBranch.mutate({
						projectId,
						subjectBranch: operation.subjectBranch,
						targetBranch: operation.targetBranch,
						dryRun: operation.dryRun,
					});
				},
				TearOffBranch: (operation) => {
					tearOffBranch.mutate({
						projectId,
						subjectBranch: operation.subjectBranch,
						dryRun: operation.dryRun,
					});
				},
			}),
		);
	};
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
export const rubOperation = ({
	resolvedOperationSource,
	target,
}: {
	resolvedOperationSource: ResolvedOperationSource;
	target: Item;
}): Operation | null =>
	Match.value({ resolvedOperationSource, target }).pipe(
		Match.when(
			{ resolvedOperationSource: { _tag: "Commit" }, target: { _tag: "Commit" } },
			({ resolvedOperationSource, target }) =>
				commitSquashOperation({
					sourceCommitId: resolvedOperationSource.commitId,
					destinationCommitId: target.commitId,
					dryRun: false,
				}),
		),
		Match.when(
			{ resolvedOperationSource: { _tag: "Commit" }, target: { _tag: "ChangesSection" } },
			({ resolvedOperationSource }) =>
				commitUncommitOperation({
					commitId: resolvedOperationSource.commitId,
					assignTo: null,
				}),
		),
		Match.when(
			{
				resolvedOperationSource: { _tag: "TreeChanges", parent: { _tag: "Change" } },
				target: { _tag: "Commit" },
			},
			({ resolvedOperationSource, target }) =>
				commitAmendOperation({
					commitId: target.commitId,
					changes: resolvedOperationSource.changes.map(({ change, hunkHeaders }) =>
						createDiffSpec(change, hunkHeaders),
					),
					dryRun: false,
				}),
		),
		Match.when(
			{
				resolvedOperationSource: { _tag: "TreeChanges", parent: { _tag: "Commit" } },
				target: { _tag: "ChangesSection" },
			},
			({ resolvedOperationSource }) =>
				commitUncommitChangesOperation({
					commitId: resolvedOperationSource.parent.commitId,
					assignTo: null,
					changes: resolvedOperationSource.changes.map(({ change, hunkHeaders }) =>
						createDiffSpec(change, hunkHeaders),
					),
					dryRun: false,
				}),
		),
		Match.when(
			{
				resolvedOperationSource: { _tag: "TreeChanges", parent: { _tag: "Commit" } },
				target: { _tag: "Commit" },
			},
			({ resolvedOperationSource, target }) =>
				commitMoveChangesBetweenOperation({
					sourceCommitId: resolvedOperationSource.parent.commitId,
					destinationCommitId: target.commitId,
					changes: resolvedOperationSource.changes.map(({ change, hunkHeaders }) =>
						createDiffSpec(change, hunkHeaders),
					),
					dryRun: false,
				}),
		),
		Match.orElse(() => null),
	);

export const moveOperation = ({
	resolvedOperationSource,
	target,
	side,
}: {
	resolvedOperationSource: ResolvedOperationSource;
	target: Item;
	side: InsertSide;
}) => {
	const relativeTo: RelativeTo | null = Match.value(target).pipe(
		Match.withReturnType<RelativeTo | null>(),
		Match.tags({
			Commit: ({ commitId }) => ({ type: "commit", subject: commitId }),
			Branch: ({ branchRef }) => ({ type: "referenceBytes", subject: branchRef }),
		}),
		Match.orElse(() => null),
	);

	return Match.value({ resolvedOperationSource, target, relativeTo }).pipe(
		// This should support `relativeTo`:
		// https://linear.app/gitbutler/issue/GB-1161/refsbranches-should-use-bytes-instead-of-strings
		// https://linear.app/gitbutler/issue/GB-1199/support-moving-branches-onto-commits
		// https://linear.app/gitbutler/issue/GB-1232/support-moving-branch-before-another-branch
		Match.when(
			{ resolvedOperationSource: { _tag: "Branch" }, relativeTo: { type: "referenceBytes" } },
			({ resolvedOperationSource, relativeTo }) =>
				moveBranchOperation({
					subjectBranch: decodeRefName(resolvedOperationSource.branchRef),
					targetBranch: decodeRefName(relativeTo.subject),
					dryRun: false,
				}),
		),
		Match.when(
			{
				resolvedOperationSource: { _tag: "Branch" },
				target: { _tag: "BaseCommit" },
			},
			({ resolvedOperationSource }) =>
				tearOffBranchOperation({
					subjectBranch: decodeRefName(resolvedOperationSource.branchRef),
					dryRun: false,
				}),
		),
		Match.whenOr(
			{ resolvedOperationSource: { _tag: "Commit" }, relativeTo: Match.defined },
			({ resolvedOperationSource: { commitId }, relativeTo }) =>
				commitMoveOperation({
					subjectCommitIds: [commitId],
					relativeTo,
					side,
					dryRun: false,
				}),
		),
		Match.when(
			{
				resolvedOperationSource: { _tag: "TreeChanges", parent: { _tag: "Change" } },
				relativeTo: Match.defined,
			},
			({ resolvedOperationSource, relativeTo }) =>
				commitCreateOperation({
					relativeTo,
					side,
					changes: resolvedOperationSource.changes.map(({ change, hunkHeaders }) =>
						createDiffSpec(change, hunkHeaders),
					),
					message: "",
					dryRun: false,
				}),
		),
		Match.when(
			{
				resolvedOperationSource: { _tag: "TreeChanges", parent: { _tag: "Commit" } },
				relativeTo: Match.defined,
			},
			({ resolvedOperationSource, relativeTo }) =>
				commitCreateFromCommittedChangesOperation({
					sourceCommitId: resolvedOperationSource.parent.commitId,
					relativeTo,
					side,
					changes: resolvedOperationSource.changes.map(({ change, hunkHeaders }) =>
						createDiffSpec(change, hunkHeaders),
					),
					dryRun: false,
				}),
		),
		Match.orElse(() => null),
	);
};
