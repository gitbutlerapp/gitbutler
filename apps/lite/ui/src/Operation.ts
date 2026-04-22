import { Toast, UseToastManagerReturnValue } from "@base-ui/react";
import { QueryClient, useMutation, useQueryClient } from "@tanstack/react-query";
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
import { CommitUncommitParams } from "#ui/api/mutations.ts";
import { InsertSide, RelativeTo } from "@gitbutler/but-sdk";
import { Item, itemParent } from "#ui/routes/project/$id/workspace/Item.ts";
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

const runOperation =
	({
		queryClient,
		toastManager,
	}: {
		queryClient: QueryClient;
		toastManager: UseToastManagerReturnValue;
	}) =>
	({
		projectId,
		operation,
		dryRun,
	}: {
		projectId: string;
		operation: Operation;
		dryRun: boolean;
	}) =>
		Match.value(operation).pipe(
			Match.tagsExhaustive({
				CommitAmend: async (operation) => {
					const changes = resolveDiffSpecs({
						source: operation.source,
						queryClient,
						projectId,
					});
					if (!changes) return;

					const response = await window.lite.commitAmend({
						projectId,
						commitId: operation.commitId,
						changes,
						dryRun,
					});

					if (!dryRun && response.rejectedChanges.length > 0)
						toastManager.add(
							rejectedChangesToastOptions({
								newCommit: response.newCommit,
								rejectedChanges: response.rejectedChanges,
							}),
						);
				},
				CommitMoveChangesBetween: async (operation) => {
					const changes = resolveDiffSpecs({
						source: operation.source,
						queryClient,
						projectId,
					});
					if (!changes) return;

					await window.lite.commitMoveChangesBetween({
						projectId,
						sourceCommitId: operation.sourceCommitId,
						destinationCommitId: operation.destinationCommitId,
						changes,
						dryRun,
					});
				},
				CommitSquash: async (operation) => {
					await window.lite.commitSquash({
						projectId,
						sourceCommitId: operation.sourceCommitId,
						destinationCommitId: operation.destinationCommitId,
						dryRun,
					});
				},
				CommitUncommit: async () => {
					throw new Error("Uncommitting has not been implemented yet.");
				},
				CommitUncommitChanges: async (operation) => {
					const changes = resolveDiffSpecs({
						source: operation.source,
						queryClient,
						projectId,
					});
					if (!changes) return;

					await window.lite.commitUncommitChanges({
						projectId,
						commitId: operation.commitId,
						assignTo: operation.assignTo,
						changes,
						dryRun,
					});
				},
				CommitCreate: async (operation) => {
					const changes = resolveDiffSpecs({
						source: operation.source,
						queryClient,
						projectId,
					});
					if (!changes) return;

					const response = await window.lite.commitCreate({
						projectId,
						relativeTo: operation.relativeTo,
						side: operation.side,
						changes,
						message: operation.message,
						dryRun,
					});

					if (!dryRun && response.rejectedChanges.length > 0)
						toastManager.add(
							rejectedChangesToastOptions({
								newCommit: response.newCommit,
								rejectedChanges: response.rejectedChanges,
							}),
						);
				},
				CommitCreateFromCommittedChanges: async (operation) => {
					const changes = resolveDiffSpecs({
						source: operation.source,
						queryClient,
						projectId,
					});
					if (!changes) return;

					// Ideally this would be an atomic backend operation.
					const insertedCommit = await window.lite.commitInsertBlank({
						projectId,
						relativeTo: operation.relativeTo,
						side: operation.side,
						dryRun,
					});

					await window.lite.commitMoveChangesBetween({
						projectId,
						sourceCommitId:
							insertedCommit.workspace.replacedCommits[operation.sourceCommitId] ??
							operation.sourceCommitId,
						destinationCommitId: insertedCommit.newCommit,
						changes,
						dryRun,
					});
				},
				CommitMove: async (operation) => {
					await window.lite.commitMove({
						projectId,
						subjectCommitIds: operation.subjectCommitIds,
						relativeTo: operation.relativeTo,
						side: operation.side,
						dryRun,
					});
				},
				MoveBranch: async (operation) => {
					await window.lite.moveBranch({
						projectId,
						subjectBranch: operation.subjectBranch,
						targetBranch: operation.targetBranch,
						dryRun,
					});
				},
				TearOffBranch: async (operation) => {
					await window.lite.tearOffBranch({
						projectId,
						subjectBranch: operation.subjectBranch,
						dryRun,
					});
				},
			}),
		);

export const useRunOperation = () => {
	const toastManager = Toast.useToastManager();
	const queryClient = useQueryClient();
	const runOperationMutation = useMutation({
		mutationFn: runOperation({ queryClient, toastManager }),
		onSuccess: async (_data, _input, _ctx, { client }) => {
			await client.invalidateQueries();
		},
	});

	return (projectId: string, operation: Operation): void => {
		runOperationMutation.mutate({ projectId, operation, dryRun: false });
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
	source,
	target,
}: {
	source: Item;
	target: Item;
}): Operation | null =>
	Match.value({ source, sourceParent: itemParent(source), target }).pipe(
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
				sourceParent: { _tag: "Change" },
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
				sourceParent: { _tag: "Commit" },
				target: { _tag: "ChangesSection" },
			},
			({ source, sourceParent }) =>
				commitUncommitChangesOperation({
					commitId: sourceParent.commitId,
					assignTo: null,
					source,
				}),
		),
		Match.when(
			{
				sourceParent: { _tag: "Commit" },
				target: { _tag: "Commit" },
			},
			({ source, sourceParent, target }) =>
				commitMoveChangesBetweenOperation({
					sourceCommitId: sourceParent.commitId,
					destinationCommitId: target.commitId,
					source,
				}),
		),
		Match.orElse(() => null),
	);

export const moveOperation = ({
	source,
	target,
	side,
}: {
	source: Item;
	target: Item;
	side: InsertSide;
}) => {
	const branchMoveOperation = Match.value({ source, target }).pipe(
		// This should support `relativeTo`:
		// https://linear.app/gitbutler/issue/GB-1161/refsbranches-should-use-bytes-instead-of-strings
		// https://linear.app/gitbutler/issue/GB-1199/support-moving-branches-onto-commits
		// https://linear.app/gitbutler/issue/GB-1232/support-moving-branch-before-another-branch
		Match.when(
			{
				source: { _tag: "Branch" },
				target: { _tag: "Branch" },
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

	return Match.value({ source, sourceParent: itemParent(source) }).pipe(
		Match.when({ source: { _tag: "Commit" } }, ({ source }) =>
			commitMoveOperation({
				subjectCommitIds: [source.commitId],
				relativeTo,
				side,
			}),
		),
		Match.when({ sourceParent: { _tag: "Change" } }, ({ source }) =>
			commitCreateOperation({
				relativeTo,
				side,
				source,
				message: "",
			}),
		),
		Match.when({ sourceParent: { _tag: "Commit" } }, ({ source, sourceParent }) =>
			commitCreateFromCommittedChangesOperation({
				sourceCommitId: sourceParent.commitId,
				relativeTo,
				side,
				source,
			}),
		),
		Match.orElse(() => null),
	);
};
