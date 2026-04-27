import { Toast } from "@base-ui/react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
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
import { Item, itemEquals, itemFileParent } from "#ui/routes/project/$id/workspace/Item.ts";
import { resolveDiffSpecs } from "#ui/routes/project/$id/workspace/resolveDiffSpecs.ts";
import { decodeRefName } from "#ui/routes/project/$id/shared.tsx";

/** @public */
export type CommitAmendOperation = Omit<CommitAmendParams, "dryRun" | "projectId" | "changes"> & {
	source: Item;
};
/** @public */
export type CommitCreateOperation = Omit<CommitCreateParams, "dryRun" | "projectId" | "changes"> & {
	source: Item;
};
/** @public */
export type CommitCreateFromCommittedChangesOperation = Omit<
	CommitInsertBlankParams,
	"dryRun" | "projectId"
> &
	Pick<CommitMoveChangesBetweenParams, "sourceCommitId"> & {
		source: Item;
	};
/** @public */
export type CommitMoveOperation = Omit<CommitMoveParams, "dryRun" | "projectId">;
/** @public */
export type CommitMoveChangesBetweenOperation = Omit<
	CommitMoveChangesBetweenParams,
	"dryRun" | "projectId" | "changes"
> & { source: Item };
/** @public */
export type CommitSquashOperation = Omit<CommitSquashParams, "dryRun" | "projectId">;
/** @public */
export type CommitUncommitOperation = Omit<CommitUncommitParams, "dryRun" | "projectId">;
/** @public */
export type CommitUncommitChangesOperation = Omit<
	CommitUncommitChangesParams,
	"dryRun" | "projectId" | "changes"
> & { source: Item };
/** @public */
export type MoveBranchOperation = Omit<MoveBranchParams, "dryRun" | "projectId">;
/** @public */
export type TearOffBranchOperation = Omit<TearOffBranchParams, "dryRun" | "projectId">;

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
	const queryClient = useQueryClient();
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
					const changes = resolveDiffSpecs({
						source: operation.source,
						queryClient,
						projectId,
					});
					if (!changes) return;

					commitAmend.mutate(
						{
							projectId,
							commitId: operation.commitId,
							changes,
							dryRun: false,
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
					const changes = resolveDiffSpecs({
						source: operation.source,
						queryClient,
						projectId,
					});
					if (!changes) return;

					commitMoveChangesBetween.mutate({
						projectId,
						sourceCommitId: operation.sourceCommitId,
						destinationCommitId: operation.destinationCommitId,
						changes,
						dryRun: false,
					});
				},
				CommitSquash: (operation) => {
					commitSquash.mutate({
						projectId,
						sourceCommitId: operation.sourceCommitId,
						destinationCommitId: operation.destinationCommitId,
						dryRun: false,
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
					const changes = resolveDiffSpecs({
						source: operation.source,
						queryClient,
						projectId,
					});
					if (!changes) return;

					commitUncommitChanges.mutate({
						projectId,
						commitId: operation.commitId,
						assignTo: operation.assignTo,
						changes,
						dryRun: false,
					});
				},
				CommitCreate: (operation) => {
					const changes = resolveDiffSpecs({
						source: operation.source,
						queryClient,
						projectId,
					});
					if (!changes) return;

					commitCreate.mutate(
						{
							projectId,
							relativeTo: operation.relativeTo,
							side: operation.side,
							changes,
							message: operation.message,
							dryRun: false,
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
					const changes = resolveDiffSpecs({
						source: operation.source,
						queryClient,
						projectId,
					});
					if (!changes) return;

					// Ideally this would be an atomic backend operation.
					void (async () => {
						const insertedCommit = await commitInsertBlank.mutateAsync({
							projectId,
							relativeTo: operation.relativeTo,
							side: operation.side,
							dryRun: false,
						});

						await commitMoveChangesBetween.mutateAsync({
							projectId,
							sourceCommitId:
								insertedCommit.workspace.replacedCommits[operation.sourceCommitId] ??
								operation.sourceCommitId,
							destinationCommitId: insertedCommit.newCommit,
							changes,
							dryRun: false,
						});
					})();
				},
				CommitMove: (operation) => {
					commitMove.mutate({
						projectId,
						subjectCommitIds: operation.subjectCommitIds,
						relativeTo: operation.relativeTo,
						side: operation.side,
						dryRun: false,
					});
				},
				MoveBranch: (operation) => {
					moveBranch.mutate({
						projectId,
						subjectBranch: operation.subjectBranch,
						targetBranch: operation.targetBranch,
						dryRun: false,
					});
				},
				TearOffBranch: (operation) => {
					tearOffBranch.mutate({
						projectId,
						subjectBranch: operation.subjectBranch,
						dryRun: false,
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
const rubOperation = ({ source, target }: { source: Item; target: Item }): Operation | null =>
	Match.value({ source, sourceFileParent: itemFileParent(source), target }).pipe(
		Match.when(
			{
				source: { _tag: "Commit" },
				target: { _tag: "Commit" },
			},
			({ source, target }) =>
				commitSquashOperation({
					sourceCommitId: source.commitId,
					destinationCommitId: target.commitId,
				}),
		),
		Match.when(
			{
				source: { _tag: "Commit" },
				target: { _tag: "ChangesSection" },
			},
			({ source }) =>
				commitUncommitOperation({
					commitId: source.commitId,
					assignTo: null,
				}),
		),
		Match.when(
			{
				sourceFileParent: { _tag: "Changes" },
				target: { _tag: "Commit" },
			},
			({ source, target }) =>
				commitAmendOperation({
					commitId: target.commitId,
					source,
				}),
		),
		Match.when(
			{
				sourceFileParent: { _tag: "Commit" },
				target: { _tag: "ChangesSection" },
			},
			({ source, sourceFileParent }) =>
				commitUncommitChangesOperation({
					commitId: sourceFileParent.commitId,
					assignTo: null,
					source,
				}),
		),
		Match.when(
			{
				sourceFileParent: { _tag: "Commit" },
				target: { _tag: "Commit" },
			},
			({ source, sourceFileParent, target }) =>
				commitMoveChangesBetweenOperation({
					sourceCommitId: sourceFileParent.commitId,
					destinationCommitId: target.commitId,
					source,
				}),
		),
		Match.orElse(() => null),
	);

const moveOperation = ({
	source,
	target,
	side,
}: {
	source: Item;
	target: Item;
	side: InsertSide;
}) => {
	const branchMoveOperation = Match.value({ source, target, side }).pipe(
		// This should support `relativeTo`:
		// https://linear.app/gitbutler/issue/GB-1161/refsbranches-should-use-bytes-instead-of-strings
		// https://linear.app/gitbutler/issue/GB-1199/support-moving-branches-onto-commits
		// https://linear.app/gitbutler/issue/GB-1232/support-moving-branch-before-another-branch
		Match.when(
			{
				source: { _tag: "Branch" },
				target: { _tag: "Branch" },
				side: "above",
			},
			({ source, target }) =>
				moveBranchOperation({
					subjectBranch: decodeRefName(source.branchRef),
					targetBranch: decodeRefName(target.branchRef),
				}),
		),
		Match.when(
			{
				source: { _tag: "Branch" },
				target: { _tag: "BaseCommit" },
				side: "above",
			},
			({ source }) =>
				tearOffBranchOperation({
					subjectBranch: decodeRefName(source.branchRef),
				}),
		),
		Match.orElse(() => null),
	);

	if (branchMoveOperation) return branchMoveOperation;

	const relativeTo: RelativeTo | null = Match.value(target).pipe(
		Match.withReturnType<RelativeTo | null>(),
		Match.tags({
			Commit: ({ commitId }) => ({ type: "commit", subject: commitId }),
			Branch: ({ branchRef }) => ({ type: "referenceBytes", subject: branchRef }),
		}),
		Match.orElse(() => null),
	);

	if (!relativeTo) return null;

	return Match.value({ source, sourceFileParent: itemFileParent(source) }).pipe(
		Match.when({ source: { _tag: "Commit" } }, ({ source }) =>
			commitMoveOperation({
				subjectCommitIds: [source.commitId],
				relativeTo,
				side,
			}),
		),
		Match.when({ sourceFileParent: { _tag: "Changes" } }, ({ source }) =>
			commitCreateOperation({
				relativeTo,
				side,
				source,
				message: "",
			}),
		),
		Match.when({ sourceFileParent: { _tag: "Commit" } }, ({ source, sourceFileParent }) =>
			commitCreateFromCommittedChangesOperation({
				sourceCommitId: sourceFileParent.commitId,
				relativeTo,
				side,
				source,
			}),
		),
		Match.orElse(() => null),
	);
};

export type OperationType = "rub" | "moveAbove" | "moveBelow";

const isOperationSourceEnabled = (source: Item): boolean =>
	Match.value(source).pipe(
		Match.when({ _tag: "Hunk", isResultOfBinaryToTextConversion: true }, () => false),
		Match.orElse(() => true),
	);

export const getOperations = (
	source: Item,
	target: Item,
): Record<OperationType, Operation | null> => {
	if (itemEquals(source, target) || !isOperationSourceEnabled(source))
		return {
			rub: null,
			moveAbove: null,
			moveBelow: null,
		};
	return {
		rub: rubOperation({ source, target }),
		moveAbove: moveOperation({ source, target, side: "above" }),
		moveBelow: moveOperation({ source, target, side: "below" }),
	};
};

export const getOperation = (x: {
	source: Item;
	target: Item;
	operationType: OperationType | null;
}): Operation | null => {
	const { rub, moveAbove, moveBelow } = getOperations(x.source, x.target);
	return Match.value(x.operationType).pipe(
		Match.when(null, () => null),
		Match.when("rub", () => rub),
		Match.when("moveAbove", () => moveAbove),
		Match.when("moveBelow", () => moveBelow),
		Match.exhaustive,
	);
};
