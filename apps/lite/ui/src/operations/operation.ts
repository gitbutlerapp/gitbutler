import { Toast } from "@base-ui/react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
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
	CommitUncommitParams,
} from "#electron/ipc.ts";
import { headInfoQueryOptions } from "#ui/api/queries.ts";
import { rejectedChangesToastOptions } from "#ui/operations/rejectedChangesToastOptions.tsx";
import { DiffSpec, InsertSide, RelativeTo } from "@gitbutler/but-sdk";
import { Operand, operandEquals, operandFileParent } from "#ui/operands.ts";
import { resolveDiffSpecs, useResolveDiffSpecs } from "#ui/operations/diff-specs.ts";
import { decodeRefName } from "#ui/api/ref-name.ts";
import { projectActions } from "#ui/projects/state.ts";
import { useAppDispatch } from "#ui/store.ts";
import { useParams } from "@tanstack/react-router";
import { errorMessageForToast } from "#ui/errors.ts";

/** @public */
export type CommitAmendOperation = Omit<CommitAmendParams, "dryRun" | "projectId" | "changes"> & {
	source: Operand;
};
/** @public */
export type CommitCreateOperation = Omit<CommitCreateParams, "dryRun" | "projectId" | "changes"> & {
	source: Operand;
};
/** @public */
export type CommitSplitOperation = Omit<CommitInsertBlankParams, "dryRun" | "projectId"> &
	Pick<CommitMoveChangesBetweenParams, "sourceCommitId"> & {
		source: Operand;
	};
/** @public */
export type CommitMoveOperation = Omit<CommitMoveParams, "dryRun" | "projectId"> & {
	source: Operand;
};
/** @public */
export type CommitMoveChangesBetweenOperation = Omit<
	CommitMoveChangesBetweenParams,
	"dryRun" | "projectId" | "changes"
> & { source: Operand };
/** @public */
export type CommitSquashOperation = Omit<CommitSquashParams, "dryRun" | "projectId"> & {
	source: Operand;
};
/** @public */
export type CommitUncommitOperation = Omit<CommitUncommitParams, "dryRun" | "projectId"> & {
	source: Operand;
};
/** @public */
export type CommitUncommitChangesOperation = Omit<
	CommitUncommitChangesParams,
	"dryRun" | "projectId" | "changes"
> & { source: Operand };
/** @public */
export type MoveBranchOperation = Omit<MoveBranchParams, "dryRun" | "projectId"> & {
	source: Operand;
};
/** @public */
export type TearOffBranchOperation = Omit<TearOffBranchParams, "dryRun" | "projectId"> & {
	source: Operand;
};

export type Operation =
	| ({ _tag: "CommitAmend" } & CommitAmendOperation)
	| ({ _tag: "CommitCreate" } & CommitCreateOperation)
	| ({ _tag: "CommitSplit" } & CommitSplitOperation)
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
export const commitSplitOperation = (operation: CommitSplitOperation): Operation => ({
	_tag: "CommitSplit",
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
			CommitSplit: ({ side }) =>
				Match.value(side).pipe(
					Match.when("above", () => "Create commit above"),
					Match.when("below", () => "Create commit below"),
					Match.exhaustive,
				),
			CommitMove: ({ side }) =>
				Match.value(side).pipe(
					Match.when("above", () => "Move above"),
					Match.when("below", () => "Move below"),
					Match.exhaustive,
				),
			CommitMoveChangesBetween: () => "Amend",
			CommitSquash: () => "Squash",
			CommitUncommit: () => "Uncommit",
			CommitUncommitChanges: () => "Uncommit",
			MoveBranch: () => "Move above",
			TearOffBranch: () => "Tear off branch",
		}),
	);

const runOperation = async ({
	projectId,
	operation,
	resolveChanges,
	dryRun,
}: {
	projectId: string;
	operation: Operation;
	resolveChanges: (source: Operand) => Promise<Array<DiffSpec> | null>;
	dryRun: boolean;
}) =>
	Match.value(operation).pipe(
		Match.tagsExhaustive({
			CommitAmend: async (operation) => {
				const changes = await resolveChanges(operation.source);
				if (!changes) return null;
				return window.lite.commitAmend({
					projectId,
					commitId: operation.commitId,
					changes,
					dryRun,
				});
			},
			CommitMoveChangesBetween: async (operation) => {
				const changes = await resolveChanges(operation.source);
				if (!changes) return null;
				return window.lite.commitMoveChangesBetween({
					projectId,
					sourceCommitId: operation.sourceCommitId,
					destinationCommitId: operation.destinationCommitId,
					changes,
					dryRun,
				});
			},
			CommitSquash: (operation) =>
				window.lite.commitSquash({
					projectId,
					sourceCommitIds: operation.sourceCommitIds,
					destinationCommitId: operation.destinationCommitId,
					dryRun,
				}),
			CommitUncommit: (operation) =>
				window.lite.commitUncommit({
					projectId,
					subjectCommitIds: operation.subjectCommitIds,
					assignTo: operation.assignTo,
					dryRun,
				}),
			CommitUncommitChanges: async (operation) => {
				const changes = await resolveChanges(operation.source);
				if (!changes) return null;
				return window.lite.commitUncommitChanges({
					projectId,
					commitId: operation.commitId,
					assignTo: operation.assignTo,
					changes,
					dryRun,
				});
			},
			CommitCreate: async (operation) => {
				const changes = await resolveChanges(operation.source);
				if (!changes) return null;
				return window.lite.commitCreate({
					projectId,
					relativeTo: operation.relativeTo,
					side: operation.side,
					changes,
					message: operation.message,
					dryRun,
				});
			},
			CommitSplit: async (operation) => {
				const changes = await resolveChanges(operation.source);
				if (!changes) return null;

				// We can't dry run this as it's not an atomic operation. Ideally this
				// would be an atomic backend operation.
				if (dryRun) return null;

				const insertedCommit = await window.lite.commitInsertBlank({
					projectId,
					relativeTo: operation.relativeTo,
					side: operation.side,
					dryRun,
				});

				return window.lite.commitMoveChangesBetween({
					projectId,
					sourceCommitId:
						insertedCommit.workspace.replacedCommits[operation.sourceCommitId] ??
						operation.sourceCommitId,
					destinationCommitId: insertedCommit.newCommit,
					changes,
					dryRun,
				});
			},
			CommitMove: (operation) =>
				window.lite.commitMove({
					projectId,
					subjectCommitIds: operation.subjectCommitIds,
					relativeTo: operation.relativeTo,
					side: operation.side,
					dryRun,
				}),
			MoveBranch: (operation) =>
				window.lite.moveBranch({
					projectId,
					subjectBranch: operation.subjectBranch,
					targetBranch: operation.targetBranch,
					dryRun,
				}),
			TearOffBranch: (operation) =>
				window.lite.tearOffBranch({
					projectId,
					subjectBranch: operation.subjectBranch,
					dryRun,
				}),
		}),
	);

export const useDryRunOperation = ({
	projectId,
	operation,
}: {
	projectId: string;
	operation?: Operation;
}) => {
	const changes = useResolveDiffSpecs({
		projectId,
		operand: operation ? operation.source : undefined,
	});

	return useQuery({
		enabled: !!operation,
		queryKey: ["dryRun", projectId, operation, changes],
		queryFn: () => {
			if (!operation) return null;
			return runOperation({
				projectId,
				operation,
				resolveChanges: async () => changes,
				dryRun: true,
			});
		},
		// We may have a lot of different dry runs in a short amount of time.
		gcTime: 10_000,
	});
};

export const useRunOperation = () => {
	const { id: projectId } = useParams({ from: "/project/$id/workspace" });
	const dispatch = useAppDispatch();
	const queryClient = useQueryClient();
	const toastManager = Toast.useToastManager();

	return useMutation({
		mutationFn: (operation: Operation) =>
			runOperation({
				projectId,
				operation,
				resolveChanges: (source) => resolveDiffSpecs({ projectId, queryClient, source }),
				dryRun: false,
			}),
		onSuccess: async (response, _input, _ctx, { client }) => {
			if (response) {
				client.setQueryData(headInfoQueryOptions(projectId).queryKey, response.workspace.headInfo);
				dispatch(
					projectActions.updateRewrittenCommitReferences({
						projectId,
						replacedCommits: response.workspace.replacedCommits,
						headInfo: response.workspace.headInfo,
					}),
				);

				if ("rejectedChanges" in response && response.rejectedChanges.length > 0)
					toastManager.add(
						rejectedChangesToastOptions({
							newCommit: response.newCommit,
							rejectedChanges: response.rejectedChanges,
						}),
					);
			}

			await client.invalidateQueries();
		},
		onError: (error) => {
			// oxlint-disable-next-line no-console
			console.error(error);

			toastManager.add({
				type: "error",
				title: "Failed to run operation",
				description: errorMessageForToast(error),
				priority: "high",
			});
		},
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
const rubOperation = ({ source, target }: { source: Operand; target: Operand }): Operation | null =>
	Match.value({ source, sourceFileParent: operandFileParent(source), target }).pipe(
		Match.when(
			{
				source: { _tag: "Commit" },
				target: { _tag: "Commit" },
			},
			({ source, target }) =>
				commitSquashOperation({
					source,
					sourceCommitIds: [source.commitId],
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
					source,
					subjectCommitIds: [source.commitId],
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
	source: Operand;
	target: Operand;
	side: InsertSide;
}) => {
	const branchMoveOperation = Match.value({ source, target, side }).pipe(
		// This should support `relativeTo`:
		// https://linear.app/gitbutler/issue/GB-1161/refsbranches-should-use-bytes-instead-of-strings
		// https://linear.app/gitbutler/issue/GB-1199/support-moving-branches-relative-to-commits
		// https://linear.app/gitbutler/issue/GB-1232/support-moving-branch-before-another-branch
		Match.when(
			{
				source: { _tag: "Branch" },
				target: { _tag: "Branch" },
				side: "above",
			},
			({ source, target }) =>
				moveBranchOperation({
					source,
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
					source,
					subjectBranch: decodeRefName(source.branchRef),
				}),
		),
		Match.orElse(() => null),
	);

	if (branchMoveOperation) return branchMoveOperation;

	const relativeTo: RelativeTo | null = Match.value(target).pipe(
		Match.tags({
			Commit: ({ commitId }): RelativeTo | null => ({ type: "commit", subject: commitId }),
			Branch: ({ branchRef }): RelativeTo | null => ({
				type: "referenceBytes",
				subject: branchRef,
			}),
		}),
		Match.orElse((): RelativeTo | null => null),
	);

	if (!relativeTo) return null;

	return Match.value({ source, sourceFileParent: operandFileParent(source) }).pipe(
		Match.when({ source: { _tag: "Commit" } }, ({ source }) =>
			commitMoveOperation({
				source,
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
			commitSplitOperation({
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

const isOperationSourceEnabled = (source: Operand): boolean =>
	Match.value(source).pipe(
		Match.when({ _tag: "Hunk", isResultOfBinaryToTextConversion: true }, () => false),
		Match.orElse(() => true),
	);

export type OperationsByType = Record<OperationType, Operation | null>;

export const getOperations = (source: Operand, target: Operand): OperationsByType => {
	if (operandEquals(source, target) || !isOperationSourceEnabled(source))
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
	source: Operand;
	target: Operand;
	operationType: OperationType;
}): Operation | null => {
	const { rub, moveAbove, moveBelow } = getOperations(x.source, x.target);
	return Match.value(x.operationType).pipe(
		Match.when("rub", () => rub),
		Match.when("moveAbove", () => moveAbove),
		Match.when("moveBelow", () => moveBelow),
		Match.exhaustive,
	);
};
